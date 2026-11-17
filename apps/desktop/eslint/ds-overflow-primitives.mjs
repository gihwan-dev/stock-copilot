import { projectPath } from "./types.mjs";
import { POSITION_FILES, SCROLL_FILES } from "./ds-overflow.mjs";
export const PRIMITIVE_OWNERS = [
  ["ScrollArea", "scroll-area", "ScrollArea"],
  ["Tooltip", "tooltip", "Tooltip"],
  ["Popover", "popover", "Popover"],
  ["Menu", "menu", "Menu"],
  ["ContextMenu", "context-menu", "Menu"],
  ["NavigationMenu", "navigation-menu", "Menu"],
  ["PreviewCard", "preview-card", "Popover"],
  ["Dialog", "dialog", "Dialog"],
  ["AlertDialog", "alert-dialog", "Dialog"],
  ["Toast", "toast", "Toast"],
  ["Select", "select", "Select"],
  ["Combobox", "combobox", "Combobox"],
  ["Autocomplete", "autocomplete", "Combobox"],
  ["Drawer", "drawer", null],
  ["Slider", "slider", null],
  ["NumberField", "number-field", null],
];
export const PRIMITIVE_EXCEPTIONS = [
  [
    "src/shared/ds/IconButton.tsx",
    "@base-ui/react/tooltip",
    "Tooltip.Positioner",
  ],
];
const tooltipParts = [
  "Provider",
  "Root",
  "Trigger",
  "Portal",
  "Popup",
  "Positioner",
];
export const primitiveOverflowVisitors = (ctx) => {
  const file = projectPath(ctx, ctx.filename);
  const icon = file === PRIMITIVE_EXCEPTIONS[0][0];
  const tooltipSource = PRIMITIVE_EXCEPTIONS[0][1];
  const report = (node) => ctx.report({ node, messageId: "forbidden" });
  const owned = (entry) =>
    entry[2] !== null &&
    file === `src/shared/ds/${entry[2]}.tsx` &&
    (entry[0] === "ScrollArea" ? SCROLL_FILES : POSITION_FILES).includes(file);
  const family = (source) =>
    PRIMITIVE_OWNERS.find(
      ([, path]) =>
        source === `@base-ui/react/${path}` ||
        source.startsWith(`@base-ui/react/${path}/`),
    );
  const exactTooltipImport = (node) =>
    icon &&
    node.type === "ImportDeclaration" &&
    node.source.value === tooltipSource &&
    node.specifiers.length === 1 &&
    node.specifiers[0].type === "ImportSpecifier" &&
    node.specifiers[0].imported.name === "Tooltip" &&
    node.specifiers[0].local.name === "Tooltip";
  const sourceBoundary = (node) => {
    const source = node.source?.value;
    if (typeof source !== "string") return;
    const entry = family(source);
    if (
      entry &&
      (!owned(entry) || node.type !== "ImportDeclaration") &&
      !exactTooltipImport(node)
    )
      report(node);
    if (source === "@base-ui/react") {
      if (node.type !== "ImportDeclaration" || node.specifiers.length === 0) {
        report(node);
        return;
      }
      for (const specifier of node.specifiers) {
        const entry = PRIMITIVE_OWNERS.find(
          ([name]) =>
            name === (specifier.imported?.name ?? specifier.imported?.value),
        );
        if (specifier.type !== "ImportSpecifier" || (entry && !owned(entry)))
          report(specifier);
      }
    }
  };
  const tooltipBinding =
    icon && ctx.sourceCode.ast.body.some(exactTooltipImport);
  let positioners = 0;
  return {
    ImportDeclaration: sourceBoundary,
    ExportNamedDeclaration: sourceBoundary,
    ExportAllDeclaration: sourceBoundary,
    ImportExpression(node) {
      if (
        typeof node.source.value !== "string" ||
        node.source.value.startsWith("@base-ui/react")
      )
        report(node);
    },
    Identifier(node) {
      if (
        tooltipBinding &&
        node.name === "Tooltip" &&
        node.parent.type !== "ImportSpecifier"
      )
        report(node);
    },
    JSXOpeningElement(node) {
      if (!tooltipBinding) return;
      const name = node.name;
      if (name.type === "JSXIdentifier" && name.name === "Tooltip")
        report(node);
      if (name.type !== "JSXMemberExpression" || name.object.name !== "Tooltip")
        return;
      if (!tooltipParts.includes(name.property.name)) report(node);
      if (name.property.name === "Positioner" && ++positioners > 1)
        report(node);
    },
  };
};
