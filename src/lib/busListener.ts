import { apiUrl } from "@/api/server";

/** "audio/L16; rate=24000; channels=1" -> 24000 Hz, 1 channel (48 kHz stereo when not said). */
export function pcmFormat(contentType: string | null): { rate: number; channels: 1 | 2 } {
  const rate = Number(/rate=(\d+)/.exec(contentType ?? "")?.[1]);
  const channels = /channels=(\d)/.exec(contentType ?? "")?.[1] === "1" ? 1 : 2;
  return { rate: rate >= 8_000 && rate <= 96_000 ? rate : 48_000, channels };
}

/**
 * Plays sound as the server sends it: a bus (/audio/buses/:n/listen) or a translation track
 * (/translation/tracks/:language/listen). Raw 16-bit PCM; the rate and channels are in the
 * answer's Content-Type (48 kHz stereo, or less for speech). It plays a little behind so short
 * network hiccups don't break it up.
 */
export class BusListener {
  private ctx: AudioContext | null = null;
  private gain: GainNode | null = null;
  private abort: AbortController | null = null;
  private next = 0;
  private volume = 1;
  /** Called when it starts, stops or fails ("" = no problem). */
  onState: (playing: boolean, error: string) => void = () => {};

  /** How far behind the server it plays (s): steadier on Wi-Fi, later than the room. */
  static readonly LEAD = 0.25;

  get playing(): boolean {
    return this.abort !== null;
  }

  setVolume(v: number) {
    this.volume = v;
    if (this.gain) this.gain.gain.value = v;
  }

  /** Starts listening to bus `n` (call from a tap: browsers only start sound after one). */
  start(n: number, mono = false) {
    return this.startPath(`/audio/buses/${n}/listen`, mono ? { mono: "1" } : {});
  }

  /** Starts listening to any of the server's sound paths (also from a tap). */
  async startPath(path: string, params: Record<string, string> = {}) {
    this.stop();
    const ctx = (this.ctx ??= new AudioContext({ sampleRate: 48_000 }));
    await ctx.resume().catch(() => undefined);
    if (!this.gain) {
      this.gain = ctx.createGain();
      this.gain.connect(ctx.destination);
    }
    this.gain.gain.value = this.volume;
    const abort = new AbortController();
    this.abort = abort;
    this.next = 0;
    this.onState(true, "");
    try {
      const res = await fetch(apiUrl(path, params), { credentials: "include", cache: "no-store", signal: abort.signal });
      if (!res.ok || !res.body) throw new Error(await refusal(res));
      const { rate, channels } = pcmFormat(res.headers.get("content-type"));
      const reader = res.body.getReader();
      let carry = new Uint8Array(0);
      for (;;) {
        const { value, done } = await reader.read();
        if (done || abort.signal.aborted) break;
        // Samples are 2 bytes; a message may split one.
        const bytes = new Uint8Array(carry.length + value.length);
        bytes.set(carry);
        bytes.set(value, carry.length);
        const whole = bytes.length - (bytes.length % (2 * channels));
        carry = bytes.slice(whole);
        this.play(ctx, new DataView(bytes.buffer, 0, whole), rate, channels);
      }
      if (!abort.signal.aborted) throw new Error("the server stopped sending");
    } catch (e) {
      if (abort.signal.aborted) return;
      this.abort = null;
      this.onState(false, e instanceof Error ? e.message : String(e));
    }
  }

  stop() {
    if (!this.abort) return;
    this.abort.abort();
    this.abort = null;
    this.onState(false, "");
  }

  private play(ctx: AudioContext, data: DataView, rate: number, channels: number) {
    const frames = data.byteLength / (2 * channels);
    if (!frames || !this.gain) return;
    // The context resamples buffers at another rate.
    const buffer = ctx.createBuffer(2, frames, rate);
    const left = buffer.getChannelData(0);
    const right = buffer.getChannelData(1);
    for (let i = 0; i < frames; i++) {
      left[i] = data.getInt16(i * 2 * channels, true) / 32768;
      right[i] = channels === 2 ? data.getInt16(i * 4 + 2, true) / 32768 : left[i];
    }
    const now = ctx.currentTime;
    // Too far behind (a stall) or not started: start again a little ahead.
    if (this.next < now || this.next > now + 1) this.next = now + BusListener.LEAD;
    const source = ctx.createBufferSource();
    source.buffer = buffer;
    source.connect(this.gain);
    source.start(this.next);
    this.next += buffer.duration;
  }
}

/** Why the server didn't send sound: its own words when it gives them ("Spanish translation: off"). */
async function refusal(res: Response): Promise<string> {
  if (res.status === 401) return "log in again";
  const said = await res
    .json()
    .then((j: { error?: unknown }) => (typeof j.error === "string" ? j.error : ""))
    .catch(() => "");
  return said.replace(/^conflict: /, "") || `the server answered ${res.status}`;
}
