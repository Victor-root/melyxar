/*
 * A length in seconds, chosen from the usual ones or typed in by hand.
 *
 * Typed in by hand stays open once asked for, even on a length that happens
 * to be one of the usual ones: somebody typing 20 has not asked for the
 * field to close under their fingers.
 */

import { useState } from "react";
import { NumberField, Picker } from "../../components/panel";
import { offeredAs, STEP_LENGTHS, TYPED_IN } from "../../player/steps";
import { useSettings } from "../../settings";

export function LengthPicker({
  label,
  seconds,
  longest,
  noneOffered = false,
  onPick,
}: {
  label: string;
  seconds: number;
  longest: number;
  /** Whether nought, "none", is one of the choices. */
  noneOffered?: boolean;
  onPick: (seconds: number) => void;
}) {
  const { t } = useSettings();
  const offered = offeredAs(seconds, noneOffered);
  const [typing, setTyping] = useState(offered === TYPED_IN);
  const options: [string, string][] = [
    ...(noneOffered ? [["0", t("settings.length_none")] as [string, string]] : []),
    ...STEP_LENGTHS.map((one): [string, string] => [
      String(one),
      t("settings.step_seconds", { seconds: one }),
    ]),
    [TYPED_IN, t("settings.length_typed_in")],
  ];

  return (
    <>
      <Picker
        label={label}
        value={typing ? TYPED_IN : offered}
        options={options}
        onPick={(picked) => {
          setTyping(picked === TYPED_IN);
          if (picked !== TYPED_IN) {
            onPick(Number(picked));
          }
        }}
      />
      {typing && (
        <span className="field-with-unit">
          <NumberField
            label={t("settings.length_in_seconds", { name: label })}
            value={seconds}
            min={noneOffered ? 0 : 1}
            max={longest}
            onPick={onPick}
          />
          <span className="field-unit" aria-hidden="true">
            {t("settings.seconds_unit")}
          </span>
        </span>
      )}
    </>
  );
}
