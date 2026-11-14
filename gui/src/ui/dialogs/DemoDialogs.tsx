// File > Open Example, and the web demo's welcome (first visit in a browser, Help > Web Demo).

import { openExample } from "../../actions";
import { LANGUAGES, t, type Lang } from "../../i18n";
import { BROWSER_MAX_JOINTS } from "../../ipc/backend";
import { EXAMPLES } from "../../model/examples";
import { usePrefs } from "../../state/prefs";
import { useStore } from "../../state/store";
import { Dialog } from "../Dialog";

const close = () => useStore.getState().openDialog(null);

/** The example models, each with an Open button. */
function ExampleList() {
  return (
    <ul className="example-list">
      {EXAMPLES.map((ex) => (
        <li key={ex.id}>
          <button onClick={() => void openExample(ex.id)}>{t(ex.title)}</button>
          <span className="hint note">{t(ex.description)}</span>
        </li>
      ))}
    </ul>
  );
}

export function ExamplesDialog() {
  return (
    <Dialog title={t("demo.examplesTitle")} wide buttons={<button onClick={close}>{t("dlg.cancel")}</button>}>
      <p>{t("demo.examplesHint")}</p>
      <ExampleList />
    </Dialog>
  );
}

export function WelcomeDialog() {
  const lang = usePrefs((p) => p.lang);
  return (
    <Dialog
      title={t("demo.welcomeTitle")}
      wide
      buttons={
        <>
          <button onClick={() => useStore.getState().openDialog("new")}>{t("demo.newModel")}</button>
          <button className="primary" onClick={close}>
            {t("dlg.close")}
          </button>
        </>
      }
    >
      <div className="welcome">
        <label className="row">
          <span>{t("md2.language")}</span>
          <select aria-label={t("md2.language")} value={lang} onChange={(e) => usePrefs.getState().set({ lang: e.target.value as Lang })}>
            {LANGUAGES.map((l) => (
              <option key={l.code} value={l.code}>
                {l.name}
              </option>
            ))}
          </select>
        </label>
        <p>{t("demo.welcomeLine1")}</p>
        <p>{t("demo.welcomeLine2", { limit: BROWSER_MAX_JOINTS })}</p>
        <p>{t("md2.aboutWarning")}</p>
        <h4>{t("demo.examplesHeading")}</h4>
        <p className="hint">{t("demo.examplesHint")}</p>
        <ExampleList />
      </div>
    </Dialog>
  );
}
