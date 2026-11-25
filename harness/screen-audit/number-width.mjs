export const inspectNumberWidths = (doc) => {
  const findings = [];
  const loaded = [...doc.fonts]
    .filter((face) => face.status === "loaded")
    .map((face) => face.family.replaceAll('"', ""));
  if (!loaded.includes("IBM Plex Sans KR"))
    findings.push({ kind: "font-not-loaded", detail: loaded.join(",") });
  const view = doc.defaultView;
  const measure = (source, text) => {
    const probe = doc.createElement("span");
    const style = view.getComputedStyle(source);
    probe.style.font = style.font;
    probe.style.fontVariantNumeric = style.fontVariantNumeric;
    probe.style.letterSpacing = style.letterSpacing;
    probe.style.whiteSpace = "pre";
    probe.style.position = "absolute";
    probe.style.visibility = "hidden";
    probe.textContent = text;
    doc.body.append(probe);
    const width = probe.getBoundingClientRect().width;
    probe.remove();
    return width;
  };
  const numbers = [...doc.querySelectorAll(".tabular-nums")].filter((node) =>
    /\d/u.test(node.textContent ?? ""),
  );
  if (numbers.length === 0)
    findings.push({ kind: "no-numbers", detail: doc.location.search });
  for (const node of numbers) {
    const text = node.textContent ?? "";
    const narrow = measure(node, text.replace(/\d/gu, "1"));
    const wide = measure(node, text.replace(/\d/gu, "8"));
    if (Math.abs(narrow - wide) > 0.5)
      findings.push({
        kind: "proportional-digits",
        detail: `${text} ${narrow.toFixed(2)}/${wide.toFixed(2)}`,
      });
  }
  return { count: numbers.length, findings };
};
