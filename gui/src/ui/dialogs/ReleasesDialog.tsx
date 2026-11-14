// Assign > Frame Releases: end releases (P V2 V3 T M2 M3 at end i and end j) of the selected
// members.

import { useState } from "react";

import { t, tn, type Key } from "../../i18n";
import { setFrameReleases } from "../../model/ops";
import { RELEASES, type Release, type Releases } from "../../model/types";
import { useStore } from "../../state/store";
import { Dialog } from "../Dialog";

const close = () => useStore.getState().openDialog(null);

type Ticks = Record<"i" | "j", Record<Release, boolean>>;

const ticksOf = (r?: Releases): Ticks => ({
  i: Object.fromEntries(RELEASES.map((x) => [x, !!r?.i?.includes(x)])) as Record<Release, boolean>,
  j: Object.fromEntries(RELEASES.map((x) => [x, !!r?.j?.includes(x)])) as Record<Release, boolean>,
});

export function ReleasesDialog() {
  const model = useStore((s) => s.model);
  const frames = useStore((s) => s.selection.frames);
  const own = frames.map((id) => model.frames.find((e) => e.id === id)?.releases);
  const key = (r?: Releases) => JSON.stringify(ticksOf(r));
  const mixed = new Set(own.map(key)).size > 1;
  const [ticks, setTicks] = useState<Ticks>(() => ticksOf(own[0]));
  const apply = (ok: boolean) => {
    const releases: Releases = {
      i: RELEASES.filter((r) => ticks.i[r]),
      j: RELEASES.filter((r) => ticks.j[r]),
    };
    if (useStore.getState().edit((m) => setFrameReleases(m, frames, releases)) && ok) close();
  };
  const preset = (pinned: boolean) => {
    const set = (r: Release) => pinned && (r === "M2" || r === "M3");
    setTicks({
      i: Object.fromEntries(RELEASES.map((r) => [r, set(r)])) as Record<Release, boolean>,
      j: Object.fromEntries(RELEASES.map((r) => [r, set(r)])) as Record<Release, boolean>,
    });
  };
  return (
    <Dialog
      title={tn("rel.title", frames.length)}
      buttons={
        <>
          <button onClick={() => preset(true)}>{t("rel.pinned")}</button>
          <button onClick={() => preset(false)}>{t("rel.fixed")}</button>
          <button className="primary" onClick={() => apply(true)}>
            {t("dlg.ok")}
          </button>
          <button onClick={() => apply(false)}>{t("dlg.apply")}</button>
          <button onClick={close}>{t("dlg.cancel")}</button>
        </>
      }
    >
      <table className="releases">
        <thead>
          <tr>
            <th />
            <th>{t("rel.start")}</th>
            <th>{t("rel.end")}</th>
          </tr>
        </thead>
        <tbody>
          {RELEASES.map((r) => (
            <tr key={r}>
              <td>{t(`rel.${r}` as Key)}</td>
              {(["i", "j"] as const).map((end) => (
                <td key={end}>
                  <input
                    type="checkbox"
                    aria-label={t("rel.box", { r, end: t(end === "i" ? "rel.start" : "rel.end") })}
                    checked={ticks[end][r]}
                    onChange={(e) => setTicks({ ...ticks, [end]: { ...ticks[end], [r]: e.target.checked } })}
                  />
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      {mixed && <p className="hint note">{t("rel.mixed")}</p>}
      <p className="hint">{t("rel.hint")}</p>
    </Dialog>
  );
}
