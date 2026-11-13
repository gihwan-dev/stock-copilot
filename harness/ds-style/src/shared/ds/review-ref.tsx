const value = (
  <div
    className="h-meter-height bg-primary"
    ref={(el) => el?.style.setProperty("width", `${p.pct}%`)}
  />
);
