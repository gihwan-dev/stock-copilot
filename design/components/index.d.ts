import type { ReactNode, ReactElement } from 'react';

export type Currency = 'KRW' | 'USD';

/** 코어(Rust)가 계산하고 표시 자릿수까지 반올림한 소수 문자열. /^-?(0|[1-9]\d*)(\.\d+)?$/
 *  숫자 타입·"1,234"·"1e5"·"NaN"·""는 잘못된 값 → "—" 표시 + onFormatError. */
export type Decimal = string;

export interface FormatError { kind: string; input: unknown; where: string }

export interface ButtonProps {
  /** primary: 화면당 하나 · secondary: 기본 · ghost: 툴바/닫기 · buy/sell: SignalCard 전용 */
  variant?: 'primary' | 'secondary' | 'ghost' | 'buy' | 'sell';
  size?: 'sm' | 'md';
  onClick?: () => void;
  disabled?: boolean;
  type?: 'button' | 'submit';
  'aria-label'?: string;
  children: ReactNode;
}

export interface PriceChangeProps {
  /** 등락 금액 */
  value?: Decimal;
  /** 등락률 % ("2.43" = +2.43%) */
  rate?: Decimal;
  currency?: Currency;
  size?: 'sm' | 'md' | 'lg';
  /** 배지형 */
  pill?: boolean;
}

export interface BadgeProps {
  tone?: 'neutral' | 'primary' | 'up' | 'down' | 'alert';
  /** 종목 코드 등 고정폭 */
  mono?: boolean;
  children: ReactNode;
}

export interface MetricProps {
  label: ReactNode;
  /** 포맷된 값 */
  value: ReactNode;
  change?: PriceChangeProps;
  /** 기준 시각·출처 */
  hint?: ReactNode;
  /** 총 평가금액 한 곳에만 */
  display?: boolean;
}

export interface SparklineProps {
  /** 오래된 값부터 */
  /** 그리기 전용이라 숫자도 받아요 */
  data: Array<string | number>;
  /** 비교 기준 (기본: 첫 값) */
  baseline?: string | number;
  /** 코어가 정한 방향. 주면 색이 이걸 따라요 */
  direction?: 'up' | 'down' | 'flat';
  width?: number;
  height?: number;
  label?: string;
}

export interface AllocationItem {
  label: string;
  /** 비중 % (코어 계산) */
  weight: Decimal;
  /** 코어가 사용자 상한과 비교한 결과 */
  over?: boolean;
}
export interface AllocationBarProps {
  /** 큰 비중부터, 최대 6개 (기타는 코어가 묶음). 7개 이상이면 실패 Callout */
  items: AllocationItem[];
  /** 쏠림 배지에 붙는 기준 문구, 예: '상한 40%' */
  limitLabel?: string;
}

export interface HoldingRow {
  id?: string;
  name: string;
  ticker: string;
  market?: string;
  /** 행의 원래 통화. 환산하지 않음 */
  currency: Currency;
  qty: Decimal;
  avgPrice: Decimal;
  price: Decimal;
  value: Decimal;
  pl: Decimal;
  /** 수익률 % */
  rate: Decimal;
  /** 원화 환산 기준 비중 % */
  weight: Decimal;
}
export interface HoldingsTotal {
  /** 원화 환산 합계 */
  value: Decimal;
  pl: Decimal;
  rate: Decimal;
  /** 외화 종목이 있으면 채움. 합계에 ≈ 와 환율·시각이 붙음 */
  fx?: { pair: string; rate: Decimal; at: string };
}
export interface HoldingsTableProps {
  rows: HoldingRow[];
  total?: HoldingsTotal;
  /** 행 row-height-dense */
  dense?: boolean;
  selectedId?: string;
  /** 주면 행이 Tab·Enter·Space로 선택 가능, 선택 행은 aria-current */
  onSelect?: (row: HoldingRow) => void;
}

export type SegmentOption = string | { value: string; label: ReactNode };
export interface SegmentedControlProps {
  options: SegmentOption[];
  value?: string;
  defaultValue?: string;
  onChange?: (value: string) => void;
  /** 접근성 그룹 이름 */
  label?: string;
}

export interface ChatSource { label: string; href?: string }
export interface ChatMessageProps {
  role?: 'user' | 'assistant';
  /** 기본 'AI 비서' */
  name?: string;
  /** AI가 확인한 것을 사용자 말 한 줄로: '보유 종목을 확인했어요'. 내부 도구 이름 금지 */
  activity?: string;
  /** @deprecated activity를 쓰세요. 넘기면 배지가 아니라 한 줄 글자로 보여요 */
  tools?: string[];
  sources?: ChatSource[];
  /** 면책 한 줄 */
  note?: ReactNode;
  /** 답변이 들어오는 중이면 끝에 커서 */
  streaming?: boolean;
  /** assistant의 문자열은 Markdown으로 그려요 */
  children: ReactNode;
}

export interface CalloutProps {
  /** info 알림(휴장) · warning 주의(쏠림, 오래된 데이터, 외부 출처뿐) · error 실패(연결, 주문, 값 오류). 'alert'는 warning */
  tone?: 'info' | 'warning' | 'error' | 'alert';
  title?: ReactNode;
  action?: ReactNode;
  children: ReactNode;
}

export declare const Button: (p: ButtonProps) => JSX.Element;
export declare const PriceChange: (p: PriceChangeProps) => JSX.Element;
export declare const Badge: (p: BadgeProps) => JSX.Element;
export declare const Metric: (p: MetricProps) => JSX.Element;
export declare const Sparkline: (p: SparklineProps) => JSX.Element;
export declare const AllocationBar: (p: AllocationBarProps) => JSX.Element;
export declare const HoldingsTable: (p: HoldingsTableProps) => JSX.Element;
export declare const SegmentedControl: (p: SegmentedControlProps) => JSX.Element;
export declare const ChatMessage: (p: ChatMessageProps) => JSX.Element;
export declare const Callout: (p: CalloutProps) => JSX.Element;
export declare const format: {
  /** 잘못된 값이면 '—' */
  money(x: Decimal, currency?: Currency, signed?: boolean): string;
  pct(x: Decimal, signed?: boolean): string;
  /** 끝의 0만 지움: '0.5주' */
  qty(x: Decimal): string;
  num(x: Decimal): string;
  /** 근사 요약(만 단위 반올림): '1조 2,345억 6,789만원' */
  krwShort(x: Decimal): string;
  dir(x: Decimal): 'up' | 'down' | 'flat' | 'invalid';
  isValid(x: unknown): x is Decimal;
  INVALID: '—';
};
/** 잘못된 값이 표시될 때마다 호출. 반환값은 구독 해제 함수 */
export declare function onFormatError(fn: (e: FormatError) => void): () => void;

/* ================= tokens as props ================= */
/** 간격 토큰 이름만 받아요. 다른 값은 무시하고 onFormatError로 보고해요 */
export type SpaceToken = 'space-1' | 'space-2' | 'space-3' | 'space-4' | 'space-5' | 'space-6' | 'space-8' | 'space-10' | 'space-12';
export type WidthToken = 'sidebar-width' | 'sidebar-collapsed' | 'chat-width' | 'chat-width-min' | 'chat-width-max' | 'content-min' | 'dialog-width' | 'dialog-width-wide' | 'popover-min-width' | 'tooltip-max-width' | 'prose-max' | 'content-max';
export type ChartHeightToken = 'chart-height-sm' | 'chart-height' | 'chart-height-lg';
export type IconName = 'search' | 'settings' | 'x' | 'chevron-down' | 'chevron-up' | 'chevron-right' | 'chevron-left' | 'check' | 'plus' | 'minus' | 'triangle-alert' | 'circle-alert' | 'circle-x' | 'info' | 'refresh-cw' | 'send-horizontal' | 'square' | 'copy' | 'external-link' | 'plug' | 'star' | 'wallet' | 'trending-up' | 'trending-down' | 'newspaper' | 'file-text' | 'bell' | 'panel-right-open' | 'panel-right-close' | 'panel-left-open' | 'panel-left-close' | 'eye' | 'eye-off' | 'lock' | 'clock' | 'fingerprint' | 'message-square' | 'clipboard-paste' | 'ellipsis' | 'arrow-left' | 'chart-candlestick' | 'chart-pie' | 'target' | 'sunrise' | 'sunset';
/** 확률: 1–99 정수 문자열 */
export type ProbabilityString = string;

/* ================= layout (className·style 없음) ================= */
export interface AppShellProps {
  list?: ReactNode;
  chat?: ReactNode;
  toast?: ReactNode;
  chatOpen?: boolean;
  defaultChatOpen?: boolean;
  onChatOpenChange?: (open: boolean) => void;
  children: ReactNode;
}
export interface AppShellState {
  /** lg ≥ bp-window-lg · md ≥ bp-window-md · sm 그 아래 */
  windowBand: 'lg' | 'md' | 'sm';
  chatMode: 'docked' | 'overlay' | 'none';
  chatOpen: boolean;
  setChatOpen: (open: boolean) => void;
  listCollapsed: boolean;
  listOpen: boolean;
  setListOpen: (open: boolean) => void;
}
export interface PageProps { title: ReactNode; leading?: ReactNode; actions?: ReactNode; status?: ReactNode; children: ReactNode }
export interface SectionProps { title?: ReactNode; actions?: ReactNode; children: ReactNode }
export interface GridProps { children: ReactNode }
export type Span = 3 | 4 | 6 | 8 | 9 | 12;
export interface GridItemProps { span?: Span | { narrow?: Span; medium?: Span; wide?: Span }; children: ReactNode }
export interface StackProps { gap?: SpaceToken; align?: 'start' | 'center' | 'end' | 'stretch'; children: ReactNode }
export interface InlineProps { gap?: SpaceToken; align?: 'start' | 'center' | 'end' | 'baseline' | 'stretch'; justify?: 'start' | 'center' | 'end' | 'between'; wrap?: boolean; children: ReactNode }
export interface SplitProps { side?: 'start' | 'end'; size?: WidthToken; gap?: SpaceToken; align?: 'start' | 'center' | 'end' | 'stretch'; children: [ReactNode, ReactNode] }
export interface CardProps { title?: ReactNode; actions?: ReactNode; children: ReactNode }
export interface ToolbarProps { label?: string; children: ReactNode }
export interface ScrollAreaProps { direction?: 'y' | 'x' | 'both'; label?: string; children: ReactNode }
export interface DividerProps { direction?: 'horizontal' | 'vertical' }

/* ================= basics ================= */
export interface TextProps { variant?: 'body' | 'body-sm' | 'label' | 'caption' | 'code'; tone?: 'default' | 'muted' | 'subtle' | 'primary' | 'alert'; as?: 'p' | 'span' | 'div'; children: ReactNode }
export interface HeadingProps { level?: 1 | 2 | 3; id?: string; children: ReactNode }
export interface IconProps { name: IconName; size?: 'sm' | 'md' | 'lg'; label?: string }
export interface IconButtonProps { icon: IconName; label: string; onClick?: () => void; size?: 'sm' | 'md'; pressed?: boolean; expanded?: boolean; disabled?: boolean; tooltip?: boolean; tooltipSide?: 'top' | 'right' }
export interface TextFieldProps { label?: ReactNode; value?: string; defaultValue?: string; onChange?: (v: string) => void; placeholder?: string; hint?: ReactNode; error?: ReactNode; disabled?: boolean; type?: 'text' | 'secret' }
export interface SelectProps { label?: ReactNode; options: Array<string | { value: string; label: ReactNode }>; value?: string; defaultValue?: string; onChange?: (v: string) => void; hint?: ReactNode; error?: ReactNode; disabled?: boolean }
export interface SwitchProps { label: ReactNode; description?: ReactNode; checked?: boolean; defaultChecked?: boolean; onChange?: (on: boolean) => void; disabled?: boolean }
export interface CheckboxProps { label: ReactNode; description?: ReactNode; checked?: boolean; defaultChecked?: boolean; onChange?: (on: boolean) => void; disabled?: boolean }
export interface DisclosureProps { title: ReactNode; open?: boolean; defaultOpen?: boolean; onOpenChange?: (open: boolean) => void; children: ReactNode }
export interface DialogProps { open: boolean; title: ReactNode; onClose?: () => void; actions?: ReactNode; wide?: boolean; /** 미리보기 전용 */ inline?: boolean; children: ReactNode }
export interface TriggerProps { onClick: () => void; expanded: boolean; controls: string }
export interface PopoverProps { trigger: (t: TriggerProps) => ReactElement; align?: 'start' | 'end'; label?: string; defaultOpen?: boolean; children: ReactNode | ((close: () => void) => ReactNode) }
export interface MenuItem { label: ReactNode; icon?: IconName; onSelect?: () => void }
export interface MenuProps { trigger: (t: TriggerProps) => ReactElement; items: MenuItem[]; label?: string; align?: 'start' | 'end'; defaultOpen?: boolean }
export interface TooltipProps { label: ReactNode; side?: 'top' | 'right'; open?: boolean; children: ReactElement }
export interface TableColumn { key: string; header: ReactNode; kind?: 'text' | 'money' | 'percent' | 'quantity' | 'probability' | 'change'; align?: 'start' | 'end'; currency?: Currency; signed?: boolean }
export interface TableProps { columns: TableColumn[]; rows: Array<Record<string, any> & { id?: string | number; currency?: Currency }>; caption?: ReactNode; empty?: ReactNode; dense?: boolean }
export interface StatusBarProps { tone?: 'warning' | 'error' | 'info'; action?: ReactNode; /** 한 문장. 없으면 아무것도 안 그림 */ children?: ReactNode }
export interface EmptyStateProps { title: ReactNode; icon?: IconName; action?: ReactNode; children?: ReactNode }
export interface SkeletonProps { lines?: number; variant?: 'text' | 'block'; height?: ChartHeightToken }
export interface SpinnerProps { label?: ReactNode }
export interface ToastProps { tone?: 'info' | 'success' | 'error'; title?: ReactNode; onClose?: () => void; children?: ReactNode }
export interface MoneyProps { value: Decimal; currency?: Currency; signed?: boolean; size?: 'sm' | 'md' | 'lg' | 'display' }
export interface PercentProps { value: Decimal; signed?: boolean; size?: 'sm' | 'md' | 'lg' | 'display' }
export interface QuantityProps { value: Decimal; size?: 'sm' | 'md' | 'lg' }
export interface ProbabilityProps { value: ProbabilityString; size?: 'sm' | 'md' | 'lg' }
export interface MarkdownProps { source: string }
export interface ChatInputProps { onSend?: (text: string) => void; onStop?: () => void; streaming?: boolean; disabled?: boolean; placeholder?: string; label?: string; value?: string; onChange?: (v: string) => void }

/* ================= charts (drawing only; labels reuse the core's strings) ================= */
export interface ChartCommon { height?: ChartHeightToken; format?: 'money' | 'percent' | 'number'; currency?: Currency; empty?: ReactNode; label?: string }
export interface LineChartProps extends ChartCommon { series: Array<{ name: string; values: Decimal[] }>; labels?: string[] }
export interface AreaChartProps extends ChartCommon { values: Decimal[]; labels?: string[]; name?: string }
export interface DonutChartProps { items: Array<{ label: string; weight: Decimal }>; centerValue?: string; centerLabel?: string; height?: ChartHeightToken; empty?: ReactNode; label?: string }
export interface TreemapProps { items: Array<{ label: string; weight: Decimal; rate?: Decimal }>; height?: ChartHeightToken; empty?: ReactNode; label?: string }
export interface Candle { label: string; open: Decimal; high: Decimal; low: Decimal; close: Decimal }
export interface CandlestickChartProps extends ChartCommon { candles: Candle[] }
export interface ReliabilityPoint { predicted: string; actual: string; count: string; /** 코어가 판정한 점인지. false면 속 빈 점(판정 보류) */ judged: boolean }
export interface ReliabilityChartProps { points: ReliabilityPoint[]; height?: ChartHeightToken; empty?: ReactNode; label?: string }

/* ================= product ================= */
export interface ProbabilityMeterProps { probability: ProbabilityString; base?: ProbabilityString; buyThreshold?: ProbabilityString; sellThreshold?: ProbabilityString; probabilityLabel?: string }
export type SignalState = 'default' | 'ordersOff' | 'zeroQty' | 'externalOnly' | 'recordOnly' | 'pending' | 'filled' | 'rejected';
export interface SignalCardProps {
  stock: { name: string; ticker?: string; market?: string };
  side: 'buy' | 'sell';
  /** '3개월', '5일' */
  horizon?: string;
  time?: string;
  currency?: Currency;
  /** 현재가 */
  price: Decimal;
  /** 목표가와 현재가 대비 등락률(코어 계산) */
  targetPrice: Decimal; targetRate: Decimal;
  /** 손절가와 현재가 대비 등락률(코어 계산) */
  stopPrice: Decimal; stopRate: Decimal;
  /** 기간이 끝나는 날 */
  horizonEnd?: string;
  quantity: Decimal; amount: Decimal;
  /** 수수료 + 세금 */
  costs: Decimal;
  /** 주문 후 비중 % */
  weightAfter: Decimal;
  /** AI가 낸 확률: 기간 안에 손절가보다 목표가에 먼저 닿을 확률 */
  aiProbability: ProbabilityString;
  /** 적중 기록을 반영한 확률(신호 판정에 쓰는 값) */
  adjustedProbability: ProbabilityString;
  baseProbability?: ProbabilityString;
  buyThreshold: ProbabilityString; sellThreshold: ProbabilityString;
  /** judged는 코어가 정함. false면 '판정 보류 (n건)' */
  history?: { hitRate: ProbabilityString; count: string; judged: boolean };
  /** 마크다운 */
  reasons: string;
  sources?: ChatSource[];
  state?: SignalState;
  /** zeroQty·rejected의 이유 한 문장 */
  reason?: string;
  doneAt?: string;
  /** 매수·매도 버튼. 앱은 여기서 macOS 확인 창(Touch ID)을 띄워요 */
  onOrder?: () => void;
  onRetry?: () => void;
}
export interface BriefingCardProps { kind: 'morning' | 'close'; title?: ReactNode; time?: string; figures?: Array<{ label: string; value: ReactNode }>; summary: string; sources?: ChatSource[] }
export interface DisclosureCardProps { stock: { name: string }; title: string; kind?: string; held?: boolean; time?: string; summary?: string; href?: string }
export interface StockItem { id?: string; name: string; ticker?: string; /** 오늘 등락률 */ rate?: Decimal }
export interface StockListItemProps extends StockItem { selected?: boolean; compact?: boolean; onSelect?: () => void }
export interface StockListProps { held?: StockItem[]; watch?: StockItem[]; selectedId?: string; onSelect?: (item: StockItem) => void; loading?: boolean }
export interface ScorecardRow { band: string; predicted: ProbabilityString; actual: ProbabilityString; count: string; /** 코어가 정함. false면 '판정 보류' */ judged: boolean }
export interface ScorecardTableProps { rows: ScorecardRow[]; caption?: ReactNode; empty?: ReactNode }
export interface ChatPanelMessage { role: 'user' | 'assistant'; content: string; activity?: string; sources?: ChatSource[]; note?: ReactNode }
export interface ChatPanelProps { title?: ReactNode; messages: ChatPanelMessage[]; status?: 'idle' | 'looking' | 'streaming' | 'error'; activity?: string; error?: ReactNode; onSend?: (text: string) => void; onStop?: () => void; onRetry?: () => void; disabled?: boolean }

export declare const AppShell: (p: AppShellProps) => JSX.Element;
export declare function useAppShell(): AppShellState;
export declare const Page: (p: PageProps) => JSX.Element;
export declare const Section: (p: SectionProps) => JSX.Element;
export declare const Grid: (p: GridProps) => JSX.Element;
export declare const GridItem: (p: GridItemProps) => JSX.Element;
export declare const Stack: (p: StackProps) => JSX.Element;
export declare const Inline: (p: InlineProps) => JSX.Element;
export declare const Split: (p: SplitProps) => JSX.Element;
export declare const Card: (p: CardProps) => JSX.Element;
export declare const Toolbar: (p: ToolbarProps) => JSX.Element;
export declare const ScrollArea: (p: ScrollAreaProps) => JSX.Element;
export declare const Divider: (p: DividerProps) => JSX.Element;
export declare const Text: (p: TextProps) => JSX.Element;
export declare const Heading: (p: HeadingProps) => JSX.Element;
export declare const Icon: (p: IconProps) => JSX.Element;
export declare const IconButton: (p: IconButtonProps) => JSX.Element;
export declare const TextField: (p: TextFieldProps) => JSX.Element;
export declare const Select: (p: SelectProps) => JSX.Element;
export declare const Switch: (p: SwitchProps) => JSX.Element;
export declare const Checkbox: (p: CheckboxProps) => JSX.Element;
export declare const Disclosure: (p: DisclosureProps) => JSX.Element;
export declare const Dialog: (p: DialogProps) => JSX.Element | null;
export declare const Popover: (p: PopoverProps) => JSX.Element;
export declare const Menu: (p: MenuProps) => JSX.Element;
export declare const Tooltip: (p: TooltipProps) => JSX.Element;
export declare const Table: (p: TableProps) => JSX.Element;
export declare const StatusBar: (p: StatusBarProps) => JSX.Element | null;
export declare const EmptyState: (p: EmptyStateProps) => JSX.Element;
export declare const Skeleton: (p: SkeletonProps) => JSX.Element;
export declare const Spinner: (p: SpinnerProps) => JSX.Element;
export declare const Toast: (p: ToastProps) => JSX.Element;
export declare const Money: (p: MoneyProps) => JSX.Element;
export declare const Percent: (p: PercentProps) => JSX.Element;
export declare const Quantity: (p: QuantityProps) => JSX.Element;
export declare const Probability: (p: ProbabilityProps) => JSX.Element;
export declare const Markdown: (p: MarkdownProps) => JSX.Element;
export declare const ChatInput: (p: ChatInputProps) => JSX.Element;
export declare const LineChart: (p: LineChartProps) => JSX.Element;
export declare const AreaChart: (p: AreaChartProps) => JSX.Element;
export declare const DonutChart: (p: DonutChartProps) => JSX.Element;
export declare const Treemap: (p: TreemapProps) => JSX.Element;
export declare const CandlestickChart: (p: CandlestickChartProps) => JSX.Element;
export declare const ReliabilityChart: (p: ReliabilityChartProps) => JSX.Element;
export declare const ProbabilityMeter: (p: ProbabilityMeterProps) => JSX.Element;
export declare const SignalCard: (p: SignalCardProps) => JSX.Element;
export declare const BriefingCard: (p: BriefingCardProps) => JSX.Element;
export declare const DisclosureCard: (p: DisclosureCardProps) => JSX.Element;
export declare const StockListItem: (p: StockListItemProps) => JSX.Element;
export declare const StockList: (p: StockListProps) => JSX.Element;
export declare const ScorecardTable: (p: ScorecardTableProps) => JSX.Element;
export declare const ChatPanel: (p: ChatPanelProps) => JSX.Element;
export declare const icons: IconName[];
