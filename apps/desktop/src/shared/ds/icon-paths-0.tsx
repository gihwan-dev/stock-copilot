export const paths0 = {
  "arrow-left": (
    <>
      <path d="m12 19-7-7 7-7" />
      <path d="M19 12H5" />
    </>
  ),
  bell: (
    <>
      <path d="M10.268 21a2 2 0 0 0 3.464 0" />
      <path d="M3.262 15.326A1 1 0 0 0 4 17h16a1 1 0 0 0 .74-1.673C19.41 13.956 18 12.499 18 8A6 6 0 0 0 6 8c0 4.499-1.411 5.956-2.738 7.326" />
    </>
  ),
  "chart-candlestick": (
    <>
      <path d="M9 5v4" />
      <rect width={4} height={6} x={7} y={9} rx={1} />
      <path d="M9 15v2" />
      <path d="M17 3v2" />
      <rect width={4} height={8} x={15} y={5} rx={1} />
      <path d="M17 13v3" />
      <path d="M3 3v16a2 2 0 0 0 2 2h16" />
    </>
  ),
  "chart-pie": (
    <>
      <path d="M21 12c.552 0 1.005-.449.95-.998a10 10 0 0 0-8.953-8.951c-.55-.055-.998.398-.998.95v8a1 1 0 0 0 1 1z" />
      <path d="M21.21 15.89A10 10 0 1 1 8 2.83" />
    </>
  ),
  check: <path d="M20 6 9 17l-5-5" />,
  "chevron-down": <path d="m6 9 6 6 6-6" />,
  "chevron-left": <path d="m15 18-6-6 6-6" />,
  "chevron-right": <path d="m9 18 6-6-6-6" />,
  "chevron-up": <path d="m18 15-6-6-6 6" />,
  "circle-alert": (
    <>
      <circle cx={12} cy={12} r={10} />
      <line x1={12} x2={12} y1={8} y2={12} />
      <line x1={12} x2={12.01} y1={16} y2={16} />
    </>
  ),
  "circle-x": (
    <>
      <circle cx={12} cy={12} r={10} />
      <path d="m15 9-6 6" />
      <path d="m9 9 6 6" />
    </>
  ),
  "clipboard-paste": (
    <>
      <path d="M11 14h10" />
      <path d="M16 4h2a2 2 0 0 1 2 2v1.344" />
      <path d="m17 18 4-4-4-4" />
      <path d="M8 4H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 1.793-1.113" />
      <rect x={8} y={2} width={8} height={4} rx={1} />
    </>
  ),
  clock: (
    <>
      <circle cx={12} cy={12} r={10} />
      <path d="M12 6v6l4 2" />
    </>
  ),
  copy: (
    <>
      <rect width={14} height={14} x={8} y={8} rx={2} ry={2} />
      <path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2" />
    </>
  ),
  ellipsis: (
    <>
      <circle cx={12} cy={12} r={1} />
      <circle cx={19} cy={12} r={1} />
      <circle cx={5} cy={12} r={1} />
    </>
  ),
} as const;
