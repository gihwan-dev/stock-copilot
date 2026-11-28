import { z } from "zod";
import { Model, type IJsonRowNode } from "flexlayout-react";
import { dockPanels } from "../lib";
const id = z.string().min(1).max(256);

const dimension = z.number().min(0).max(65536);
const tab = z
  .object({
    type: z.literal("tab"),
    id: z.enum(dockPanels),
    pinned: z.boolean().exactOptional(),
  })
  .transform((item) => ({ ...item, name: item.id, component: item.id }));
const group = z.object({
  type: z.literal("tabgroup"),
  id,
  name: z.string().max(65536).exactOptional(),
  color: z
    .string()
    .regex(/^#[a-f0-9]{6}$/iu)
    .exactOptional(),
  opened: z.boolean().exactOptional(),
  children: z.array(tab).max(dockPanels.length),
});
const rowAttributes = z.object({
  type: z.literal("row"),
  id,
  weight: dimension.exactOptional(),
});
const tabset = rowAttributes.extend({
  type: z.literal("tabset"),
  selected: z.number().int().min(-1).max(dockPanels.length).exactOptional(),
  active: z.boolean().exactOptional(),
  maximized: z.boolean().exactOptional(),
  children: z.array(z.union([tab, group])).max(dockPanels.length),
});
const row = dockPanels.reduce<z.ZodType<IJsonRowNode>>(
  (nested) =>
    rowAttributes.extend({
      children: z.array(z.union([tabset, nested])).max(dockPanels.length),
    }),
  z.never(),
);
const coordinate = z.number().min(-65536).max(65536);
const floating = z.object({
  type: z.literal("float"),
  layout: row,
  rect: z.object({
    x: coordinate,
    y: coordinate,
    width: dimension.positive(),
    height: dimension.positive(),
  }),
});
export const widgetLayoutSchema = z.object({
  global: z
    .object({ rootOrientationVertical: z.boolean().exactOptional() })
    .exactOptional(),
  layout: row,
  subLayouts: z
    .record(id, floating)
    .refine(
      (frames) =>
        Object.keys(frames).length <= dockPanels.length &&
        !Object.hasOwn(frames, Model.MAIN_LAYOUT_ID),
    )
    .exactOptional(),
});
