import { msgText } from "../i18n";
import { useLang, useT } from "../i18n/useT";
import { useApp } from "../state/store";

/** Notice bar under the title bar: missing tools (every message on its own line) and the read-only data folder. */
export function ToolsBanner() {
  const t = useT();
  const lang = useLang();
  const tools = useApp((s) => s.tools);
  const readOnly = useApp((s) => s.readOnly);
  const toolsMissing = tools !== null && !tools.ok;

  if (!toolsMissing && !readOnly) return null;
  return (
    <>
      {toolsMissing && (
        <div role="alert" className="flex-none bg-red-600 px-5 py-2 text-[13px] text-white">
          {tools.messages.length > 0
            ? tools.messages.map((m, i) => <div key={i}>{msgText(lang, m)}</div>)
            : t("banner.toolsMissing")}
        </div>
      )}
      {readOnly && (
        <div role="status" className="flex-none bg-[var(--info-bg)] px-5 py-2 text-[13px] text-[var(--info-fg)]">
          {t("banner.readOnly")}
        </div>
      )}
    </>
  );
}
