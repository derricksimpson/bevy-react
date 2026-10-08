import type { Field, Stop, Value } from "../bevy";
import { BoolField, EnumField } from "./choice";
import { ColorField, type Rgb } from "./color";
import { NumberField } from "./NumberField";
import { RampField } from "./RampField";

/** The field for a parameter or an unlinked input, by its kind. */
export function ParamField({
  label,
  field,
  value,
  z,
  width,
  constant = false,
  onChange,
}: {
  label: string;
  field: Field;
  value: Value;
  z: number;
  /** The row's logical width (sliders and the ramp scrub across it). */
  width: number;
  /** A ramp's hard steps (its node's `constant` interpolation). */
  constant?: boolean;
  onChange: (value: Value, record: boolean) => void;
}) {
  switch (field.kind) {
    case "float":
    case "int":
      return (
        <NumberField
          label={label}
          value={value as number}
          field={field}
          z={z}
          width={width}
          onChange={onChange}
        />
      );
    case "enum":
      return (
        <EnumField
          label={label}
          value={value as string}
          options={field.options}
          z={z}
          onChange={(v) => onChange(v, true)}
        />
      );
    case "bool":
      return (
        <BoolField
          label={label}
          value={value as boolean}
          z={z}
          onChange={(v) => onChange(v, true)}
        />
      );
    case "color":
      return (
        <ColorField
          label={label}
          value={value as Rgb}
          z={z}
          onChange={onChange}
        />
      );
    case "ramp":
      return (
        <RampField
          stops={value as Stop[]}
          constant={constant}
          z={z}
          width={width}
          onChange={onChange}
        />
      );
  }
}
