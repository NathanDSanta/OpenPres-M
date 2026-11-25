import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Headphones, Languages, Mic, Smartphone, Sparkles, SlidersVertical, Square, TriangleAlert } from "lucide-react";
import { audioApi, markChanged, translationApi, useLive } from "@/api";
import Select from "@/components/ui/Select";
import { Toggle } from "@/components/ui/Toggle";
import { useBuses } from "@/hooks/useBuses";
import { BusListener } from "@/lib/busListener";
import { notifyError } from "@/state/Dialogs";
import type { TranslationLanguage, TranslationMode, TranslationTrack, TranslationTrackStatus } from "@/types";
import "@/styles/Translation.css";

/** Where people get the phone app. */
const APP_DOWNLOAD = "https://github.com/NathanDSanta/OpenPres-T/releases/latest";

/** The part of a track that's saved (without what the server adds about now). */
const saved = ({ language, on, mode, bus, source_bus }: TranslationTrack): TranslationTrack => ({ language, on, mode, bus, source_bus });

/**
 * Translation: a track per language that phones listen to with the OpenPres Translation app.
 * Manual: an interpreter speaks into a microphone on this computer, which goes to a bus (the
 * Audio page), and phones on that language hear the bus. Automatic (the computer translates
 * by itself) comes later.
 */
export default function Translation() {
  const { data: view, setData, reload } = useLive(() => translationApi.get(), [], ["translation", "audio", "config", "server"]);
  const { data: audio } = useLive(() => audioApi.get(), [], ["audio"]);
  const buses = useBuses(audio);

  // Phones come and go without anyone changing anything: ask again now and then.
  const again = useRef(reload);
  again.current = reload;
  useEffect(() => {
    const id = window.setInterval(() => again.current(), 5000);
    return () => window.clearInterval(id);
  }, []);

  if (!view) return <section className="page translation-page" />;

  const save = async (track: TranslationTrack) => {
    const tracks = view.tracks.map((t) => (t.language === track.language ? saved(track) : saved(t)));
    try {
      setData(await translationApi.replace(tracks));
      markChanged();
    } catch (err) {
      notifyError(err);
    }
  };

  return (
    <section className="page translation-page">
      <div className="page-scroll">
        <div className="translation-stack">
          {!view.reachable && (
            <div className="alert alert-warning translation-alert">
              <TriangleAlert size={15} />
              <span>
                Phones can't reach this server: it only accepts this computer. The admin turns on “Let managers on other computers and tablets connect”
                (Settings → Server & security).
              </span>
            </div>
          )}
          {view.tracks.map((track) => {
            const language = view.languages.find((l) => l.code === track.language);
            return language ? (
              <TrackCard key={track.language} track={track} language={language} busOptions={buses.options(track.bus ?? undefined)} busName={(n) => audio?.config.buses[n - 1]?.name || `Bus ${n}`} onSave={save} />
            ) : null;
          })}
          <section className="card translation-phones">
            <h2 className="card-title">
              <span>
                <Smartphone size={15} /> On the phones
              </span>
            </h2>
            <p className="muted small">
              People listen with the OpenPres Translation app (Android). On the same Wi-Fi it finds this server by itself: no address, no password. They tap
              their language and it keeps playing with the screen off.
            </p>
            <div className="page-actions">
              <button className="btn btn-small" onClick={() => window.open(APP_DOWNLOAD, "_blank")}>
                Get the app
              </button>
            </div>
          </section>
        </div>
      </div>
    </section>
  );
}

function TrackCard({
  track,
  language,
  busOptions,
  busName,
  onSave,
}: {
  track: TranslationTrackStatus;
  language: TranslationLanguage;
  busOptions: { value: string; label: string }[];
  busName: (n: number) => string;
  onSave: (track: TranslationTrack) => Promise<void>;
}) {
  const navigate = useNavigate();
  const set = (change: Partial<TranslationTrack>) => void onSave({ ...track, ...change });
  const modes: { id: TranslationMode; title: string; text: string; icon: typeof Mic; ready: boolean }[] = [
    { id: "manual", title: "Interpreter", text: "Someone translates live into a microphone on this computer.", icon: Mic, ready: true },
    { id: "automatic", title: "Automatic", text: "The computer translates the preacher by itself. Coming later.", icon: Sparkles, ready: false },
  ];

  let state: React.ReactNode;
  if (track.live) state = <span className="translation-state live">On · {track.listeners === 1 ? "1 phone listening" : `${track.listeners} phones listening`}</span>;
  else if (track.on && track.problem) state = <span className="translation-state problem">On, but phones hear nothing: {track.problem}</span>;
  else state = <span className="translation-state">Off</span>;

  return (
    <section className="card translation-track">
      <header className="translation-head">
        <div className="translation-title">
          <span className="translation-icon">
            <Languages size={18} />
          </span>
          <div>
            <h2>
              {language.name} <span className="muted">· {language.english}</span>
            </h2>
            {state}
          </div>
        </div>
        <label className="toggle-inline">
          <Toggle checked={track.on} onChange={(on) => set({ on })} />
          <span>Phones can listen</span>
        </label>
      </header>

      <div className="translation-modes" role="radiogroup" aria-label="How the translation is made">
        {modes.map((m) => (
          <button
            key={m.id}
            role="radio"
            aria-checked={track.mode === m.id}
            className={`translation-mode${track.mode === m.id ? " chosen" : ""}`}
            disabled={!m.ready && track.mode !== m.id}
            title={m.ready ? undefined : "Not available yet"}
            onClick={() => set({ mode: m.id })}
          >
            <m.icon size={16} />
            <span>
              <strong>{m.title}</strong>
              <small>{m.text}</small>
            </span>
          </button>
        ))}
      </div>

      {track.mode === "manual" ? (
        <div className="field translation-bus">
          <label>The interpreter's bus</label>
          <div className="translation-row">
            <Select
              value={track.bus === null ? null : String(track.bus)}
              options={busOptions}
              placeholder="Choose a bus"
              onChange={(v) => set({ bus: Number(v) })}
              className="translation-bus-select"
            />
            <button className="btn btn-small" onClick={() => navigate("/audio")}>
              <SlidersVertical size={13} /> Audio
            </button>
            <Monitor track={track} disabled={!track.live} />
          </div>
          <span className="muted small">
            Plug the interpreter's microphone into this computer and choose it as the input of {track.bus ? `“${busName(track.bus)}”` : "that bus"} on the Audio page.
            Phones hear only that bus, as speech (24 kHz mono).
          </span>
        </div>
      ) : (
        <div className="alert alert-info translation-alert">
          <Sparkles size={15} />
          <span>Automatic translation isn't available yet: phones hear nothing in this mode. Choose Interpreter to translate live.</span>
        </div>
      )}
    </section>
  );
}

/** Listen here to what the phones hear. */
function Monitor({ track, disabled }: { track: TranslationTrackStatus; disabled: boolean }) {
  const listener = useRef<BusListener | null>(null);
  const [playing, setPlaying] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => () => listener.current?.stop(), []);
  // The track went off (or moved): the server ends the sound; stop showing it as playing.
  useEffect(() => {
    if (disabled) listener.current?.stop();
  }, [disabled]);

  const toggle = () => {
    listener.current ??= new BusListener();
    const l = listener.current;
    l.onState = (on, why) => {
      setPlaying(on);
      setError(why);
    };
    if (playing) l.stop();
    else void l.startPath(translationApi.listenPath(track.language));
  };
  return (
    <>
      <button className="btn btn-small" disabled={disabled && !playing} onClick={toggle} title="Listen here to what the phones hear">
        {playing ? <Square size={13} /> : <Headphones size={13} />} {playing ? "Stop" : "Listen"}
      </button>
      {error && <span className="error-text small">{error}</span>}
    </>
  );
}
