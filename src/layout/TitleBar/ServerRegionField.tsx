import { useT } from "../../i18n";
import { saveServerRegion, useServerRegion, type ServerRegion } from "../../ipc/serverRegion";
import { Field, Seg } from "./settingsParts";

/** Server choice: velaterm.com, or the velaterm.cn mirror for users in mainland China. */
export function ServerRegionField() {
  const t = useT();
  const region = useServerRegion();
  return <>
    <Field label={t("settings.serverRegion")}>
      <Seg<ServerRegion> value={region}
        options={[["global", t("settings.serverRegionGlobal")], ["cn", t("settings.serverRegionChina")]]}
        onChange={saveServerRegion} />
    </Field>
    <p style={{ fontSize: 11, color: "var(--text-dim)", margin: "6px 0 12px" }}>
      {t("settings.serverRegionHint")}
    </p>
  </>;
}
