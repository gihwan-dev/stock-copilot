/* @ds-bundle: {"format": 4, "namespace": "StockCopilot", "components": [{"name": "Button"}, {"name": "PriceChange"}, {"name": "Badge"}, {"name": "Metric"}, {"name": "Sparkline"}, {"name": "AllocationBar"}, {"name": "HoldingsTable"}, {"name": "SegmentedControl"}, {"name": "ChatMessage"}, {"name": "Callout"}, {"name": "AppShell"}, {"name": "Page"}, {"name": "Section"}, {"name": "Grid"}, {"name": "GridItem"}, {"name": "Stack"}, {"name": "Inline"}, {"name": "Split"}, {"name": "Card"}, {"name": "Toolbar"}, {"name": "ScrollArea"}, {"name": "Divider"}, {"name": "Text"}, {"name": "Heading"}, {"name": "Icon"}, {"name": "IconButton"}, {"name": "TextField"}, {"name": "Select"}, {"name": "Switch"}, {"name": "Checkbox"}, {"name": "Disclosure"}, {"name": "Dialog"}, {"name": "Popover"}, {"name": "Menu"}, {"name": "Tooltip"}, {"name": "Table"}, {"name": "StatusBar"}, {"name": "EmptyState"}, {"name": "Skeleton"}, {"name": "Spinner"}, {"name": "Toast"}, {"name": "Money"}, {"name": "Percent"}, {"name": "Quantity"}, {"name": "Probability"}, {"name": "Markdown"}, {"name": "ChatInput"}, {"name": "LineChart"}, {"name": "AreaChart"}, {"name": "DonutChart"}, {"name": "Treemap"}, {"name": "CandlestickChart"}, {"name": "ReliabilityChart"}, {"name": "SignalCard"}, {"name": "ProbabilityMeter"}, {"name": "BriefingCard"}, {"name": "DisclosureCard"}, {"name": "StockListItem"}, {"name": "StockList"}, {"name": "ScorecardTable"}, {"name": "ChatPanel"}]} */
(function () {
  var React = window.React;
  var h = React.createElement;
  function cx() { return Array.prototype.filter.call(arguments, Boolean).join(' '); }
  function omit(o, keys) { var r = {}; for (var k in o) if (keys.indexOf(k) < 0) r[k] = o[k]; return r; }

  /* ---- display-only formatting ----
     The core sends every value as a decimal STRING already rounded to its display digits
     ("-1234.5", "0.0312"). Here we only validate, add the sign, group digits and add the unit.
     No rounding (except krwShort, an explicitly approximate summary) and no arithmetic.
     Anything else (numbers, "1,234", "1e5", "NaN", "", null) is INVALID: shown as "—" and reported. */
  var MINUS = '−', INVALID = '—';
  var DEC = /^-?(0|[1-9]\d*)(\.\d+)?$/;
  var listeners = [];
  function report(kind, input, where) {
    var e = { kind: kind, input: input, where: where || '' };
    listeners.forEach(function (fn) { try { fn(e); } catch (_) { } });
    if (!listeners.length && window.console) console.error('[StockCopilot] invalid ' + kind + ' value', input, where || '');
    try { window.dispatchEvent(new CustomEvent('stockcopilot:format-error', { detail: e })); } catch (_) { }
  }
  /* returns {ok, text, body, sign, dir}; ok=false → text '—' */
  function dec(x, opts) {
    opts = opts || {};
    if (typeof x !== 'string' || !DEC.test(x)) { report(opts.kind || 'decimal', x, opts.where); return { ok: false, text: INVALID, body: INVALID, sign: '', dir: 'invalid' }; }
    var neg = x.charAt(0) === '-', s = neg ? x.slice(1) : x, i = s.indexOf('.');
    var int = i < 0 ? s : s.slice(0, i), frac = i < 0 ? '' : s.slice(i + 1);
    if (opts.trim) frac = frac.replace(/0+$/, '');
    var zero = /^0*$/.test(int + frac);
    var body = int.replace(/\B(?=(\d{3})+(?!\d))/g, ',') + (frac ? '.' + frac : '');
    var sign = zero ? '' : neg ? MINUS : (opts.signed ? '+' : '');
    return { ok: true, text: sign + body, body: body, sign: sign, dir: zero ? 'flat' : neg ? 'down' : 'up', zero: zero };
  }
  function moneyParts(x, currency, signed) {
    var c = currency || 'KRW', f = dec(x, { signed: signed, kind: 'money' });
    if (!f.ok) return f;
    f.text = c === 'USD' ? f.sign + '$' + f.body : f.sign + f.body + (c === 'KRW' ? '원' : ' ' + c);
    return f;
  }
  function money(x, currency, signed) { return moneyParts(x, currency, signed).text; }
  function pctParts(x, signed) { var f = dec(x, { signed: signed, kind: 'percent' }); if (f.ok) f.text += '%'; return f; }
  function pct(x, signed) { return pctParts(x, signed).text; }
  function qty(x) { var f = dec(x, { trim: true, kind: 'quantity' }); return f.ok ? f.text + '주' : f.text; }
  function num(x) { return dec(x).text; }
  function dir(x) { return dec(x).dir; }
  /* approximate summary for prose and hints: 1조 2,345억원 · 8억 4,313만원 · 8,431만원 (rounded to 만) */
  function krwShort(x) {
    var f = dec(x, { kind: 'money' });
    if (!f.ok) return f.text;
    var s = x.replace(/^-/, ''), int = s.split('.')[0], n = BigInt(int), sign = f.sign === MINUS ? MINUS : '';
    if (n < 10000n) return sign + f.body + '원';
    var man = (n + 5000n) / 10000n, jo = man / 100000000n, eok = (man / 10000n) % 10000n, rest = man % 10000n;
    var g = function (b) { return String(b).replace(/\B(?=(\d{3})+(?!\d))/g, ','); };
    var parts = [];
    if (jo) parts.push(g(jo) + '조');
    if (eok) parts.push(g(eok) + '억');
    if (rest) parts.push(g(rest) + '만');
    return sign + parts.join(' ') + '원';
  }
  /* a formatted value as a node: invalid values get a visible, labelled marker */
  function Val(f) {
    return f.ok ? f.text : h('span', { className: 'sc-invalid', title: '값 오류', 'aria-label': '값 오류' }, INVALID);
  }
  var ARROW = { up: '▲', down: '▼', flat: '–' };
  var WORD = { up: '상승', down: '하락', flat: '보합' };

  function Button(p) {
    var variant = p.variant || 'secondary', size = p.size || 'md';
    /* only what a button needs: no className, style or arbitrary DOM props */
    return h('button', {
      type: p.type || 'button', onClick: p.onClick, disabled: p.disabled, 'aria-label': p['aria-label'],
      className: cx('sc-btn', 'sc-btn-' + variant, size === 'sm' && 'sc-btn-sm')
    }, p.children);
  }

  function PriceChange(p) {
    var c = p.currency || 'KRW', size = p.size || 'md';
    var v = p.value !== undefined ? moneyParts(p.value, c, true) : null;
    var r = p.rate !== undefined ? pctParts(p.rate, true) : null;
    if ((v && !v.ok) || (r && !r.ok)) {
      return h('span', { className: cx('sc-chg', 'sc-chg-' + size, 'sc-chg-invalid'), title: '값 오류', 'aria-label': '등락 값 오류' }, INVALID);
    }
    /* direction follows what is shown: the rate if it shows a change, else the amount */
    var d = r && r.dir !== 'flat' ? r.dir : v ? v.dir : 'flat';
    var parts = [h('span', { key: 'a', 'aria-hidden': 'true' }, ARROW[d])];
    if (v) parts.push(h('span', { key: 'v' }, v.text));
    if (r) parts.push(h('span', { key: 'r' }, v ? '(' + r.text + ')' : r.text));
    return h('span', { className: cx('sc-chg', 'sc-chg-' + size, 'sc-chg-' + d, p.pill && 'sc-chg-pill'), 'aria-label': WORD[d] + ' ' + [v && v.text, r && r.text].filter(Boolean).join(' ') }, parts);
  }

  function Badge(p) {
    return h('span', { className: cx('sc-badge', 'sc-badge-' + (p.tone || 'neutral'), p.mono && 'sc-badge-mono') }, p.children);
  }

  function Metric(p) {
    return h('div', { className: cx('sc-metric') },
      h('span', { className: 'sc-metric-label' }, p.label),
      h('span', { className: cx('sc-metric-value', p.display && 'sc-metric-display') }, p.value === INVALID ? h('span', { className: 'sc-invalid', title: '값 오류' }, INVALID) : p.value),
      p.change ? h(PriceChange, Object.assign({ size: 'sm' }, p.change)) : null,
      p.hint ? h('span', { className: 'sc-metric-hint' }, p.hint) : null);
  }

  /* drawing only: maps points to pixels, decides nothing about money */
  function Sparkline(p) {
    var data = (p.data || []).map(Number), w = p.width || 96, ht = p.height || 28, pad = 2;
    if (data.length < 2 || data.some(function (v) { return !isFinite(v); })) {
      if (data.length >= 2) report('chart', p.data, 'Sparkline');
      return h('svg', { width: w, height: ht, className: 'sc-spark', role: 'img', 'aria-label': '추세 없음' });
    }
    var min = Math.min.apply(null, data), max = Math.max.apply(null, data), span = max - min || 1;
    var base = p.baseline != null ? Number(p.baseline) : data[0];
    var y = function (v) { return pad + (ht - pad * 2) * (1 - (v - min) / span); };
    var pts = data.map(function (v, i) { return (i * (w / (data.length - 1))).toFixed(1) + ',' + y(v).toFixed(1); }).join(' ');
    var d = p.direction || (data[data.length - 1] > base ? 'up' : data[data.length - 1] < base ? 'down' : 'flat');
    var by = Math.max(pad, Math.min(ht - pad, y(base)));
    return h('svg', { width: w, height: ht, viewBox: '0 0 ' + w + ' ' + ht, className: cx('sc-spark'), role: 'img', 'aria-label': p.label || ('추세 ' + WORD[d]) },
      h('line', { x1: 0, x2: w, y1: by, y2: by, className: 'sc-spark-base', strokeWidth: 1 }),
      h('polyline', { points: pts, fill: 'none', strokeWidth: 1.5, strokeLinejoin: 'round', strokeLinecap: 'round', className: 'sc-spark-' + d }));
  }

  /* items arrive sorted, grouped ('기타'), weighted and limit-checked by the core: at most 6 */
  var MAX_SLICES = 6;
  function AllocationBar(p) {
    var items = p.items || [];
    var parsed = items.map(function (it) { return pctParts(it.weight, false); });
    var bad = items.length > MAX_SLICES ? '항목이 ' + items.length + '개예요. 최대 ' + MAX_SLICES + '개라서 코어에서 기타로 묶어야 해요.'
      : parsed.some(function (f) { return !f.ok; }) ? '비중 값을 읽을 수 없는 항목이 있어요.' : null;
    if (bad) {
      if (items.length > MAX_SLICES) report('allocation', items.length, 'AllocationBar');
      return h(Callout, { tone: 'error', title: '비중을 표시할 수 없어요' }, bad + ' 막대를 그리지 않았어요.');
    }
    var over = items.filter(function (it) { return it.over; });
    return h('div', { className: cx('sc-alloc') },
      h('div', { className: 'sc-alloc-bar', role: 'img', 'aria-label': items.map(function (it, i) { return it.label + ' ' + parsed[i].text; }).join(', ') },
        items.map(function (it, i) { return h('div', { key: it.label, className: cx('sc-alloc-seg', 'sc-c' + (i + 1)), style: { flexGrow: Number(it.weight), flexBasis: 0 } }); })),
      h('ul', { className: 'sc-alloc-legend' }, items.map(function (it, i) {
        return h('li', { key: it.label, className: 'sc-alloc-item' },
          h('span', { className: cx('sc-alloc-dot', 'sc-c' + (i + 1)) }),
          h('span', { className: 'sc-alloc-name' }, it.label),
          h('span', { className: 'sc-alloc-pct' }, parsed[i].text));
      })),
      over.length ? h('div', { className: 'sc-alloc-warn' }, h(Badge, { tone: 'alert' }, '! 쏠림' + (p.limitLabel ? ' (' + p.limitLabel + ')' : '') + ' · ' + over.map(function (it) { return it.label + ' ' + pct(it.weight); }).join(', '))) : null);
  }

  /* every value is computed and rounded by the core; each row keeps its own currency */
  function HoldingsTable(p) {
    var rows = p.rows || [], t = p.total, sel = p.selectedId;
    function pick(r) { if (p.onSelect) p.onSelect(r); }
    return h('div', { className: cx('sc-table-wrap') },
      h('table', { className: cx('sc-table', p.dense && 'sc-table-dense', p.onSelect && 'sc-table-selectable') },
        h('thead', null, h('tr', null, ['종목', '보유', '평균단가', '현재가', '평가금액', '평가손익', '비중'].map(function (x) { return h('th', { key: x, scope: 'col' }, x); }))),
        h('tbody', null, rows.map(function (r) {
          var c = r.currency || 'KRW', id = r.id || r.ticker, on = id === sel;
          var qf = dec(r.qty, { trim: true, kind: 'quantity', where: id }); if (qf.ok) qf.text += '주';
          return h('tr', {
            key: id,
            tabIndex: p.onSelect ? 0 : undefined,
            'aria-current': p.onSelect && on ? 'true' : undefined,
            'data-selected': on ? 'true' : undefined,
            onClick: p.onSelect ? function () { pick(r); } : undefined,
            onKeyDown: p.onSelect ? function (e) { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); pick(r); } } : undefined
          },
            h('th', { scope: 'row' }, h('div', { className: 'sc-name' }, h('b', null, r.name), h('span', null, r.ticker + (r.market ? ' · ' + r.market : '')))),
            h('td', null, Val(qf)),
            h('td', null, Val(moneyParts(r.avgPrice, c))),
            h('td', null, Val(moneyParts(r.price, c))),
            h('td', null, Val(moneyParts(r.value, c))),
            h('td', null, h(PriceChange, { value: r.pl, rate: r.rate, currency: c })),
            h('td', null, Val(pctParts(r.weight, false))));
        })),
        t ? h('tfoot', null,
          h('tr', null,
            h('th', { scope: 'row', colSpan: 4 }, t.fx ? '합계 (원화 환산)' : '합계'),
            h('td', null, (function () { var f = moneyParts(t.value, 'KRW'); return f.ok && t.fx ? '≈ ' + f.text : Val(f); })()),
            h('td', null, h(PriceChange, { value: t.pl, rate: t.rate, currency: 'KRW' })),
            h('td', null, '100%')),
          t.fx ? h('tr', { className: 'sc-table-fx' }, h('td', { colSpan: 7 }, '환산 환율 ' + t.fx.pair + ' ' + num(t.fx.rate) + ' · ' + t.fx.at + ' 기준 · 토스 참고 환율이라 실제 체결 환율과 다를 수 있어요')) : null) : null));
  }

  function SegmentedControl(p) {
    var first = p.options[0], st = React.useState(p.defaultValue != null ? p.defaultValue : (first && (first.value != null ? first.value : first)));
    var value = p.value != null ? p.value : st[0];
    return h('div', { className: cx('sc-seg'), role: 'group', 'aria-label': p.label },
      p.options.map(function (o) {
        var v = o.value != null ? o.value : o, l = o.label != null ? o.label : o;
        return h('button', { key: v, type: 'button', 'aria-pressed': String(v === value), onClick: function () { st[1](v); p.onChange && p.onChange(v); } }, l);
      }));
  }

  function ChatMessage(p) {
    var role = p.role || 'assistant';
    return h('div', { className: cx('sc-msg', 'sc-msg-' + role) },
      role === 'assistant' ? h('div', { className: 'sc-msg-who' }, p.name || 'AI 비서') : null,
      /* one user-language line about what the assistant looked at; never internal tool names */
      role === 'assistant' && (p.activity || (p.tools && p.tools.length)) ? h('div', { className: 'sc-msg-activity' }, h(Icon, { name: p.streaming ? 'clock' : 'check', size: 'sm' }), p.activity || p.tools.join(' · ')) : null,
      h('div', { className: 'sc-msg-bubble' }, typeof p.children === 'string' && role === 'assistant' ? h(Markdown, { source: p.children }) : p.children, p.streaming ? h('span', { className: 'sc-caret', 'aria-hidden': 'true' }) : null),
      p.sources && p.sources.length ? h('div', { className: 'sc-msg-sources' }, p.sources.map(function (s, i) {
        return s.href ? h('a', { key: i, href: s.href, target: '_blank', rel: 'noreferrer' }, s.label) : h('span', { key: i }, s.label);
      })) : null,
      p.note ? h('div', { className: 'sc-msg-note' }, p.note) : null);
  }

  /* info: 알림(휴장, 동기화 완료) · warning: 주의(쏠림, 오래된 데이터) · error: 실패(토스 연결 실패, 토큰 폐기, 주문 실패, 값 오류) */
  function Callout(p) {
    var tone = p.tone === 'alert' ? 'warning' : (p.tone || 'info');
    var lead = { info: '알림', warning: '주의', error: '실패' }[tone];
    return h('div', { className: cx('sc-callout', 'sc-callout-' + tone), role: tone === 'error' ? 'alert' : 'status' },
      h('span', { className: 'sc-callout-icon', 'aria-hidden': 'true' }, tone === 'info' ? 'i' : tone === 'warning' ? '!' : '×'),
      h('div', { className: 'sc-callout-body' },
        h('p', { className: 'sc-callout-title' }, h('span', { className: 'sc-callout-lead' }, lead), p.title ? ' · ' : null, p.title),
        h('div', null, p.children),
        p.action ? h('div', { className: 'sc-callout-action' }, p.action) : null));
  }


  /* ---------- token props: layout takes token NAMES only ---------- */
  var SPACE = ['space-1', 'space-2', 'space-3', 'space-4', 'space-5', 'space-6', 'space-8', 'space-10', 'space-12'];
  var WIDTHS = ['sidebar-width', 'sidebar-collapsed', 'chat-width', 'chat-width-min', 'chat-width-max', 'content-min', 'dialog-width', 'dialog-width-wide', 'popover-min-width', 'tooltip-max-width', 'chart-height-sm', 'chart-height', 'chart-height-lg', 'prose-max', 'content-max'];
  function tok(name, list, where) {
    if (name == null || name === 'none') return undefined;
    if (list.indexOf(name) < 0) { report('token', name, where); return undefined; }
    return 'var(--' + name + ')';
  }
  function cssLen(name) {
    try {
      var v = getComputedStyle(document.documentElement).getPropertyValue('--' + name);
      var n = parseFloat(v); return isFinite(n) ? n : null;
    } catch (_) { return null; }
  }
  function useWidth(ref) {
    var st = React.useState(0);
    React.useLayoutEffect(function () {
      var el = ref.current; if (!el) return;
      var set = function () { st[1](el.getBoundingClientRect().width); };
      set();
      if (typeof ResizeObserver === 'undefined') { window.addEventListener('resize', set); return function () { window.removeEventListener('resize', set); }; }
      var ro = new ResizeObserver(set); ro.observe(el);
      return function () { ro.disconnect(); };
    }, []);
    return st[0];
  }
  function band(width) {
    var m = cssLen('bp-content-medium') || 900, w = cssLen('bp-content-wide') || 1200;
    return width >= w ? 'wide' : width >= m ? 'medium' : 'narrow';
  }
  var uid = 0;
  function useId(prefix) { var r = React.useRef(null); if (r.current == null) r.current = (prefix || 'sc') + '-' + (++uid); return r.current; }

  /* ---------- Icon (Lucide, inlined; inherits currentColor) ---------- */
  var ICONS = {"arrow-left": "<path d=\"m12 19-7-7 7-7\" /> <path d=\"M19 12H5\" />", "bell": "<path d=\"M10.268 21a2 2 0 0 0 3.464 0\" /> <path d=\"M3.262 15.326A1 1 0 0 0 4 17h16a1 1 0 0 0 .74-1.673C19.41 13.956 18 12.499 18 8A6 6 0 0 0 6 8c0 4.499-1.411 5.956-2.738 7.326\" />", "chart-candlestick": "<path d=\"M9 5v4\" /> <rect width=\"4\" height=\"6\" x=\"7\" y=\"9\" rx=\"1\" /> <path d=\"M9 15v2\" /> <path d=\"M17 3v2\" /> <rect width=\"4\" height=\"8\" x=\"15\" y=\"5\" rx=\"1\" /> <path d=\"M17 13v3\" /> <path d=\"M3 3v16a2 2 0 0 0 2 2h16\" />", "chart-pie": "<path d=\"M21 12c.552 0 1.005-.449.95-.998a10 10 0 0 0-8.953-8.951c-.55-.055-.998.398-.998.95v8a1 1 0 0 0 1 1z\" /> <path d=\"M21.21 15.89A10 10 0 1 1 8 2.83\" />", "check": "<path d=\"M20 6 9 17l-5-5\" />", "chevron-down": "<path d=\"m6 9 6 6 6-6\" />", "chevron-left": "<path d=\"m15 18-6-6 6-6\" />", "chevron-right": "<path d=\"m9 18 6-6-6-6\" />", "chevron-up": "<path d=\"m18 15-6-6-6 6\" />", "circle-alert": "<circle cx=\"12\" cy=\"12\" r=\"10\" /> <line x1=\"12\" x2=\"12\" y1=\"8\" y2=\"12\" /> <line x1=\"12\" x2=\"12.01\" y1=\"16\" y2=\"16\" />", "circle-x": "<circle cx=\"12\" cy=\"12\" r=\"10\" /> <path d=\"m15 9-6 6\" /> <path d=\"m9 9 6 6\" />", "clipboard-paste": "<path d=\"M11 14h10\" /> <path d=\"M16 4h2a2 2 0 0 1 2 2v1.344\" /> <path d=\"m17 18 4-4-4-4\" /> <path d=\"M8 4H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 1.793-1.113\" /> <rect x=\"8\" y=\"2\" width=\"8\" height=\"4\" rx=\"1\" />", "clock": "<circle cx=\"12\" cy=\"12\" r=\"10\" /> <path d=\"M12 6v6l4 2\" />", "copy": "<rect width=\"14\" height=\"14\" x=\"8\" y=\"8\" rx=\"2\" ry=\"2\" /> <path d=\"M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2\" />", "ellipsis": "<circle cx=\"12\" cy=\"12\" r=\"1\" /> <circle cx=\"19\" cy=\"12\" r=\"1\" /> <circle cx=\"5\" cy=\"12\" r=\"1\" />", "external-link": "<path d=\"M15 3h6v6\" /> <path d=\"M10 14 21 3\" /> <path d=\"M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6\" />", "eye-off": "<path d=\"M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49\" /> <path d=\"M14.084 14.158a3 3 0 0 1-4.242-4.242\" /> <path d=\"M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143\" /> <path d=\"m2 2 20 20\" />", "eye": "<path d=\"M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0\" /> <circle cx=\"12\" cy=\"12\" r=\"3\" />", "file-text": "<path d=\"M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z\" /> <path d=\"M14 2v5a1 1 0 0 0 1 1h5\" /> <path d=\"M10 9H8\" /> <path d=\"M16 13H8\" /> <path d=\"M16 17H8\" />", "fingerprint": "<path d=\"M12 10a2 2 0 0 0-2 2c0 1.02-.1 2.51-.26 4\" /> <path d=\"M14 13.12c0 2.38 0 6.38-1 8.88\" /> <path d=\"M17.29 21.02c.12-.6.43-2.3.5-3.02\" /> <path d=\"M2 12a10 10 0 0 1 18-6\" /> <path d=\"M2 16h.01\" /> <path d=\"M21.8 16c.2-2 .131-5.354 0-6\" /> <path d=\"M5 19.5C5.5 18 6 15 6 12a6 6 0 0 1 .34-2\" /> <path d=\"M8.65 22c.21-.66.45-1.32.57-2\" /> <path d=\"M9 6.8a6 6 0 0 1 9 5.2v2\" />", "info": "<circle cx=\"12\" cy=\"12\" r=\"10\" /> <path d=\"M12 16v-4\" /> <path d=\"M12 8h.01\" />", "lock": "<rect width=\"18\" height=\"11\" x=\"3\" y=\"11\" rx=\"2\" ry=\"2\" /> <path d=\"M7 11V7a5 5 0 0 1 10 0v4\" />", "message-square": "<path d=\"M22 17a2 2 0 0 1-2 2H6.828a2 2 0 0 0-1.414.586l-2.202 2.202A.71.71 0 0 1 2 21.286V5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2z\" />", "minus": "<path d=\"M5 12h14\" />", "newspaper": "<path d=\"M15 18h-5\" /> <path d=\"M18 14h-8\" /> <path d=\"M4 22h16a2 2 0 0 0 2-2V4a2 2 0 0 0-2-2H8a2 2 0 0 0-2 2v16a2 2 0 0 1-4 0v-9a2 2 0 0 1 2-2h2\" /> <rect width=\"8\" height=\"4\" x=\"10\" y=\"6\" rx=\"1\" />", "panel-left-close": "<rect width=\"18\" height=\"18\" x=\"3\" y=\"3\" rx=\"2\" /> <path d=\"M9 3v18\" /> <path d=\"m16 15-3-3 3-3\" />", "panel-left-open": "<rect width=\"18\" height=\"18\" x=\"3\" y=\"3\" rx=\"2\" /> <path d=\"M9 3v18\" /> <path d=\"m14 9 3 3-3 3\" />", "panel-right-close": "<rect width=\"18\" height=\"18\" x=\"3\" y=\"3\" rx=\"2\" /> <path d=\"M15 3v18\" /> <path d=\"m8 9 3 3-3 3\" />", "panel-right-open": "<rect width=\"18\" height=\"18\" x=\"3\" y=\"3\" rx=\"2\" /> <path d=\"M15 3v18\" /> <path d=\"m10 15-3-3 3-3\" />", "plug": "<path d=\"M12 22v-5\" /> <path d=\"M15 8V2\" /> <path d=\"M17 8a1 1 0 0 1 1 1v4a4 4 0 0 1-4 4h-4a4 4 0 0 1-4-4V9a1 1 0 0 1 1-1z\" /> <path d=\"M9 8V2\" />", "plus": "<path d=\"M5 12h14\" /> <path d=\"M12 5v14\" />", "refresh-cw": "<path d=\"M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8\" /> <path d=\"M21 3v5h-5\" /> <path d=\"M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16\" /> <path d=\"M8 16H3v5\" />", "search": "<path d=\"m21 21-4.34-4.34\" /> <circle cx=\"11\" cy=\"11\" r=\"8\" />", "send-horizontal": "<path d=\"M3.714 3.048a.498.498 0 0 0-.683.627l2.843 7.627a2 2 0 0 1 0 1.396l-2.842 7.627a.498.498 0 0 0 .682.627l18-8.5a.5.5 0 0 0 0-.904z\" /> <path d=\"M6 12h16\" />", "settings": "<path d=\"M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915\" /> <circle cx=\"12\" cy=\"12\" r=\"3\" />", "square": "<rect width=\"18\" height=\"18\" x=\"3\" y=\"3\" rx=\"2\" />", "star": "<path d=\"M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 0 0 0 1.597-1.16z\" />", "sunrise": "<path d=\"M12 2v8\" /> <path d=\"m4.93 10.93 1.41 1.41\" /> <path d=\"M2 18h2\" /> <path d=\"M20 18h2\" /> <path d=\"m19.07 10.93-1.41 1.41\" /> <path d=\"M22 22H2\" /> <path d=\"m8 6 4-4 4 4\" /> <path d=\"M16 18a4 4 0 0 0-8 0\" />", "sunset": "<path d=\"M12 10V2\" /> <path d=\"m4.93 10.93 1.41 1.41\" /> <path d=\"M2 18h2\" /> <path d=\"M20 18h2\" /> <path d=\"m19.07 10.93-1.41 1.41\" /> <path d=\"M22 22H2\" /> <path d=\"m16 6-4 4-4-4\" /> <path d=\"M16 18a4 4 0 0 0-8 0\" />", "target": "<circle cx=\"12\" cy=\"12\" r=\"10\" /> <circle cx=\"12\" cy=\"12\" r=\"6\" /> <circle cx=\"12\" cy=\"12\" r=\"2\" />", "trending-down": "<path d=\"M16 17h6v-6\" /> <path d=\"m22 17-8.5-8.5-5 5L2 7\" />", "trending-up": "<path d=\"M16 7h6v6\" /> <path d=\"m22 7-8.5 8.5-5-5L2 17\" />", "triangle-alert": "<path d=\"m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3\" /> <path d=\"M12 9v4\" /> <path d=\"M12 17h.01\" />", "wallet": "<path d=\"M19 7V4a1 1 0 0 0-1-1H5a2 2 0 0 0 0 4h15a1 1 0 0 1 1 1v4h-3a2 2 0 0 0 0 4h3a1 1 0 0 0 1-1v-2a1 1 0 0 0-1-1\" /> <path d=\"M3 5v14a2 2 0 0 0 2 2h15a1 1 0 0 0 1-1v-4\" />", "x": "<path d=\"M18 6 6 18\" /> <path d=\"m6 6 12 12\" />"};
  function Icon(p) {
    var inner = ICONS[p.name];
    if (inner == null) report('icon', p.name, 'Icon');
    return h('svg', {
      className: 'sc-icon sc-icon-' + (p.size || 'sm'), viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinecap: 'round', strokeLinejoin: 'round',
      role: p.label ? 'img' : undefined, 'aria-label': p.label || undefined, 'aria-hidden': p.label ? undefined : 'true', focusable: 'false',
      dangerouslySetInnerHTML: { __html: inner || '' }
    });
  }

  /* ================= layout ================= */
  var ShellCtx = React.createContext({ windowBand: 'lg', chatMode: 'none', chatOpen: false, setChatOpen: function () { }, listCollapsed: false, listOpen: false, setListOpen: function () { } });
  var ListModeCtx = React.createContext('full');
  function useAppShell() { return React.useContext(ShellCtx); }

  /* list · main · chat. >= bp-window-lg: all docked. md..lg: chat overlays, closed by default. < bp-window-md: list collapses to a rail.
     The window never scrolls; main and chat scroll on their own. */
  function AppShell(p) {
    var ref = React.useRef(null), w = useWidth(ref);
    var lg = cssLen('bp-window-lg') || 1280, md = cssLen('bp-window-md') || 1080;
    var wb = !w ? 'lg' : w >= lg ? 'lg' : w >= md ? 'md' : 'sm';
    var chatMode = !p.chat ? 'none' : wb === 'lg' ? 'docked' : 'overlay';
    var cs = React.useState(null), ls = React.useState(false);
    var chatOpen = p.chatOpen != null ? p.chatOpen : (cs[0] != null ? cs[0] : (p.defaultChatOpen != null ? p.defaultChatOpen : chatMode === 'docked'));
    React.useEffect(function () { if (p.chatOpen == null) cs[1](null); }, [chatMode]);
    function setChatOpen(v) { cs[1](v); if (p.onChatOpenChange) p.onChatOpenChange(v); }
    var listCollapsed = wb === 'sm';
    var ctx = { windowBand: wb, chatMode: chatMode, chatOpen: chatOpen, setChatOpen: setChatOpen, listCollapsed: listCollapsed, listOpen: listCollapsed && ls[0], setListOpen: ls[1] };
    React.useEffect(function () {
      function onKey(e) { if (e.key === 'Escape') { if (chatMode === 'overlay' && chatOpen) setChatOpen(false); if (ls[0]) ls[1](false); } }
      window.addEventListener('keydown', onKey); return function () { window.removeEventListener('keydown', onKey); };
    });
    return h(ShellCtx.Provider, { value: ctx },
      h('div', { ref: ref, className: 'sc-shell', 'data-list': listCollapsed ? 'collapsed' : 'full', 'data-chat': chatMode, 'data-band': wb },
        p.list ? h('aside', { className: 'sc-shell-list', 'aria-label': '종목 목록' }, h(ListModeCtx.Provider, { value: listCollapsed ? 'compact' : 'full' }, p.list)) : null,
        listCollapsed && ls[0] && p.list ? h('aside', { className: 'sc-shell-list-overlay', 'aria-label': '종목 목록 펼침' }, h(ListModeCtx.Provider, { value: 'overlay' }, p.list)) : null,
        h('main', { className: 'sc-shell-main' }, p.children),
        chatMode !== 'none' && chatOpen ? h('aside', { className: 'sc-shell-chat', 'aria-label': 'AI 비서' }, p.chat) : null,
        p.toast ? h('div', { className: 'sc-shell-toast', role: 'region', 'aria-label': '알림' }, p.toast) : null));
  }

  function ScrollArea(p) {
    var d = p.direction || 'y';
    return h('div', { className: 'sc-scroll sc-scroll-' + d, tabIndex: 0, role: 'region', 'aria-label': p.label || '스크롤 영역' }, p.children);
  }

  /* one screen: header row (title, actions), optional status line, scrolling body capped at content-max */
  function Page(p) {
    return h('div', { className: 'sc-page' },
      h('header', { className: 'sc-page-head' }, p.leading || null, h(Heading, { level: 1 }, p.title), p.actions || null),
      p.status || null,
      h(ScrollArea, { label: p.title }, h('div', { className: 'sc-page-body' }, p.children)));
  }

  function Section(p) {
    var id = useId('sec');
    return h('section', { className: 'sc-section', 'aria-labelledby': p.title ? id : undefined },
      p.title || p.actions ? h('div', { className: 'sc-section-head' }, h('h2', { id: id, className: 'sc-h sc-h2' }, p.title), p.actions || null) : null,
      p.children);
  }

  var SPANS = [3, 4, 6, 8, 9, 12];
  var GridCtx = React.createContext('wide');
  function Grid(p) {
    var ref = React.useRef(null), w = useWidth(ref);
    return h(GridCtx.Provider, { value: w ? band(w) : 'wide' }, h('div', { ref: ref, className: 'sc-grid', 'data-band': w ? band(w) : undefined }, p.children));
  }
  function GridItem(p) {
    var b = React.useContext(GridCtx), s = p.span;
    var n = typeof s === 'number' ? s : s ? (b === 'wide' ? (s.wide || s.medium || s.narrow) : b === 'medium' ? (s.medium || s.narrow) : s.narrow) : 12;
    if (n == null) n = 12;
    if (SPANS.indexOf(n) < 0) { report('span', n, 'GridItem'); n = 12; }
    return h('div', { style: { gridColumn: 'span ' + n, minWidth: 0 } }, p.children);
  }

  var ALIGN = { start: 'flex-start', center: 'center', end: 'flex-end', stretch: 'stretch', baseline: 'baseline', between: 'space-between' };
  function Stack(p) {
    return h('div', { className: 'sc-stackx', style: { gap: tok(p.gap || 'space-4', SPACE, 'Stack'), alignItems: ALIGN[p.align || 'stretch'] } }, p.children);
  }
  function Inline(p) {
    return h('div', { className: 'sc-inline', style: { gap: tok(p.gap || 'space-2', SPACE, 'Inline'), alignItems: ALIGN[p.align || 'center'], justifyContent: ALIGN[p.justify || 'start'], flexWrap: p.wrap ? 'wrap' : 'nowrap' } }, p.children);
  }
  /* two panes; `side` names the fixed one, `size` its width token */
  function Split(p) {
    var kids = React.Children.toArray(p.children), side = p.side || 'start', fixed = { flex: 'none', width: tok(p.size || 'chat-width', WIDTHS, 'Split') };
    return h('div', { className: 'sc-split', style: { gap: tok(p.gap || 'space-6', SPACE, 'Split'), alignItems: ALIGN[p.align || 'stretch'] } },
      h('div', { style: side === 'start' ? fixed : undefined, className: side === 'start' ? undefined : 'sc-split-fill' }, kids[0]),
      h('div', { style: side === 'end' ? fixed : undefined, className: side === 'end' ? undefined : 'sc-split-fill' }, kids[1]));
  }
  function Card(p) {
    var id = useId('card');
    return h('section', { className: 'sc-cardx', 'aria-labelledby': p.title ? id : undefined },
      p.title || p.actions ? h('div', { className: 'sc-cardx-head' }, h('h3', { id: id, className: 'sc-h sc-h3' }, p.title), p.actions || null) : null,
      h('div', { className: 'sc-cardx-body' }, p.children));
  }
  function Toolbar(p) { return h('div', { className: 'sc-toolbar', role: 'toolbar', 'aria-label': p.label }, p.children); }
  function Divider(p) { return h('hr', { className: p.direction === 'vertical' ? 'sc-divider-v' : 'sc-divider-h', 'aria-orientation': p.direction === 'vertical' ? 'vertical' : undefined }); }

  /* ================= basics ================= */
  function Text(p) {
    var v = p.variant || 'body';
    return h(p.as || 'p', { className: cx('sc-text', 'sc-text-' + v, 'sc-tone-' + (p.tone || (v === 'caption' ? 'subtle' : 'default'))) }, p.children);
  }
  function Heading(p) {
    var l = p.level || 2;
    return h('h' + l, { className: 'sc-h sc-h' + l, id: p.id }, p.children);
  }
  function IconButton(p) {
    var btn = h('button', {
      type: 'button', className: cx('sc-iconbtn', p.size === 'sm' && 'sc-iconbtn-sm'), 'aria-label': p.label, onClick: p.onClick, disabled: p.disabled,
      'aria-pressed': p.pressed != null ? String(!!p.pressed) : undefined, 'aria-expanded': p.expanded != null ? String(!!p.expanded) : undefined
    }, h(Icon, { name: p.icon, size: p.size === 'sm' ? 'sm' : 'md' }));
    return p.tooltip === false ? btn : h(Tooltip, { label: p.label, side: p.tooltipSide }, btn);
  }

  /* ----- form ----- */
  function Field(p, control) {
    return h('div', { className: 'sc-field' },
      p.label ? h('label', { className: 'sc-field-label', htmlFor: p.id }, p.label) : null,
      control,
      p.error ? h('span', { className: 'sc-field-error', id: p.id + '-err', role: 'alert' }, p.error) : p.hint ? h('span', { className: 'sc-field-hint', id: p.id + '-hint' }, p.hint) : null);
  }
  /* type 'secret': masked, never shown in full; paste button reads the clipboard */
  function TextField(p) {
    var id = useId('tf'), secret = p.type === 'secret', show = React.useState(false), inner = React.useState(p.defaultValue || '');
    var value = p.value != null ? p.value : inner[0];
    function set(v) { inner[1](v); if (p.onChange) p.onChange(v); }
    function paste() {
      try { navigator.clipboard.readText().then(function (t) { set(t.trim()); }, function () { }); } catch (_) { }
    }
    var input = h('input', {
      id: id, className: cx('sc-input', secret && !show[0] && 'sc-input-secret'), type: secret && !show[0] ? 'password' : 'text', value: value, placeholder: p.placeholder,
      disabled: p.disabled, autoComplete: secret ? 'off' : undefined, spellCheck: secret ? false : undefined,
      'aria-invalid': p.error ? 'true' : undefined, 'aria-describedby': p.error ? id + '-err' : p.hint ? id + '-hint' : undefined,
      onChange: function (e) { set(e.target.value); }
    });
    var wrap = h('div', { className: 'sc-input-wrap', 'data-invalid': p.error ? 'true' : undefined, 'data-disabled': p.disabled ? 'true' : undefined },
      input,
      secret && value ? h(IconButton, { icon: show[0] ? 'eye-off' : 'eye', label: show[0] ? '가리기' : '보기', size: 'sm', onClick: function () { show[1](!show[0]); } }) : null,
      secret ? h(Button, { size: 'sm', variant: 'ghost', onClick: paste, disabled: p.disabled }, h(Icon, { name: 'clipboard-paste' }), '붙여넣기') : null);
    return Field({ id: id, label: p.label, hint: p.hint, error: p.error }, wrap);
  }
  function Select(p) {
    var id = useId('sel');
    return Field({ id: id, label: p.label, hint: p.hint, error: p.error },
      h('div', { className: 'sc-select-wrap' },
        h('select', { id: id, className: 'sc-select', value: p.value, defaultValue: p.value == null ? p.defaultValue : undefined, disabled: p.disabled, onChange: function (e) { if (p.onChange) p.onChange(e.target.value); } },
          (p.options || []).map(function (o) { var v = o.value != null ? o.value : o; return h('option', { key: v, value: v }, o.label != null ? o.label : o); })),
        h(Icon, { name: 'chevron-down' })));
  }
  function Switch(p) {
    var id = useId('sw'), st = React.useState(!!p.defaultChecked), on = p.checked != null ? p.checked : st[0];
    function toggle() { if (p.disabled) return; st[1](!on); if (p.onChange) p.onChange(!on); }
    return h('div', { className: 'sc-switch-row' },
      h('span', { className: 'sc-switch-text' },
        h('label', { className: 'sc-field-label', id: id + '-l', htmlFor: id }, p.label),
        p.description ? h('span', { className: 'sc-field-hint', id: id + '-d' }, p.description) : null),
      h('button', { id: id, type: 'button', role: 'switch', className: 'sc-switch', 'aria-checked': String(on), 'aria-labelledby': id + '-l', 'aria-describedby': p.description ? id + '-d' : undefined, disabled: p.disabled, onClick: toggle }));
  }
  function Checkbox(p) {
    var id = useId('cb');
    return h('label', { className: 'sc-check-row', htmlFor: id },
      h('input', { id: id, type: 'checkbox', className: 'sc-checkbox', checked: p.checked, defaultChecked: p.checked == null ? p.defaultChecked : undefined, disabled: p.disabled, onChange: function (e) { if (p.onChange) p.onChange(e.target.checked); } }),
      h('span', { className: 'sc-switch-text' }, h('span', { className: 'sc-field-label' }, p.label), p.description ? h('span', { className: 'sc-field-hint' }, p.description) : null));
  }

  /* ----- disclosure & overlays ----- */
  function Disclosure(p) {
    var id = useId('disc'), st = React.useState(!!p.defaultOpen), open = p.open != null ? p.open : st[0];
    return h('div', { className: 'sc-disc' },
      h('button', { type: 'button', className: 'sc-disc-btn', 'aria-expanded': String(open), 'aria-controls': id, onClick: function () { st[1](!open); if (p.onOpenChange) p.onOpenChange(!open); } },
        h(Icon, { name: 'chevron-right' }), p.title),
      open ? h('div', { id: id, className: 'sc-disc-body' }, p.children) : null);
  }
  /* modal; Esc and the close button call onClose. `inline` renders in place (showcase only) */
  function Dialog(p) {
    var id = useId('dlg'), ref = React.useRef(null);
    React.useEffect(function () {
      if (!p.open) return;
      var el = ref.current; var f = el && el.querySelector('input, select, textarea, button:not([aria-label="닫기"])'); if (f && !p.inline) f.focus();
      function onKey(e) { if (e.key === 'Escape' && p.onClose) p.onClose(); }
      window.addEventListener('keydown', onKey); return function () { window.removeEventListener('keydown', onKey); };
    }, [p.open]);
    if (!p.open) return null;
    return h('div', { className: cx('sc-overlay', p.inline && 'sc-overlay-inline'), onMouseDown: function (e) { if (e.target === e.currentTarget && p.onClose) p.onClose(); } },
      h('div', { ref: ref, className: cx('sc-dialog', p.wide && 'sc-dialog-wide'), role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': id },
        h('div', { className: 'sc-dialog-head' }, h(Heading, { level: 2, id: id }, p.title), p.onClose ? h(IconButton, { icon: 'x', label: '닫기', onClick: p.onClose, tooltip: false }) : null),
        h('div', { className: 'sc-dialog-body' }, p.children),
        p.actions ? h('div', { className: 'sc-dialog-foot' }, p.actions) : null));
  }
  function useOutside(ref, open, close) {
    React.useEffect(function () {
      if (!open) return;
      function onDown(e) { if (ref.current && !ref.current.contains(e.target)) close(); }
      function onKey(e) { if (e.key === 'Escape') close(); }
      document.addEventListener('mousedown', onDown); window.addEventListener('keydown', onKey);
      return function () { document.removeEventListener('mousedown', onDown); window.removeEventListener('keydown', onKey); };
    }, [open]);
  }
  /* trigger: function(props) returning the trigger element, so it gets aria-expanded and onClick */
  function Popover(p) {
    var ref = React.useRef(null), st = React.useState(!!p.defaultOpen), open = st[0], id = useId('pop');
    useOutside(ref, open, function () { st[1](false); });
    var trig = p.trigger({ onClick: function () { st[1](!open); }, expanded: open, controls: id });
    return h('span', { ref: ref, className: 'sc-pop-anchor' }, trig,
      open ? h('div', { id: id, className: 'sc-popover', 'data-align': p.align || 'start', role: p.role || 'dialog', 'aria-label': p.label }, typeof p.children === 'function' ? p.children(function () { st[1](false); }) : p.children) : null);
  }
  function Menu(p) {
    return h(Popover, { trigger: p.trigger, align: p.align || 'end', role: 'presentation', defaultOpen: p.defaultOpen }, function (close) {
      return h('div', { className: 'sc-menu', role: 'menu', 'aria-label': p.label }, (p.items || []).map(function (it, i) {
        return h('button', { key: i, type: 'button', role: 'menuitem', className: 'sc-menu-item', onClick: function () { close(); if (it.onSelect) it.onSelect(); } },
          it.icon ? h(Icon, { name: it.icon }) : null, it.label);
      }));
    });
  }
  function Tooltip(p) {
    var st = React.useState(false), id = useId('tip');
    var child = React.Children.only(p.children);
    return h('span', { className: 'sc-tip-anchor', onMouseEnter: function () { st[1](true); }, onMouseLeave: function () { st[1](false); }, onFocus: function () { st[1](true); }, onBlur: function () { st[1](false); } },
      React.cloneElement(child, { 'aria-describedby': st[0] && !child.props['aria-label'] ? id : undefined }),
      st[0] || p.open ? h('span', { id: id, role: 'tooltip', className: 'sc-tooltip', 'data-side': p.side || 'top' }, p.label) : null);
  }

  /* ----- general table: values are strings from the core; kind picks the formatter ----- */
  function cellFor(kind, v, row, col) {
    var c = row.currency || col.currency || 'KRW';
    if (kind === 'money') return Val(moneyParts(v, c, col.signed));
    if (kind === 'percent') return Val(pctParts(v, col.signed));
    if (kind === 'quantity') { var q = dec(v, { trim: true, kind: 'quantity' }); if (q.ok) q.text += '주'; return Val(q); }
    if (kind === 'probability') return h(Probability, { value: v });
    if (kind === 'change') return h(PriceChange, Object.assign({ currency: c }, v));
    return v;
  }
  function Table(p) {
    var cols = p.columns || [], rows = p.rows || [];
    return h('div', { className: 'sc-table-wrap' },
      h('table', { className: cx('sc-table', p.dense && 'sc-table-dense') },
        p.caption ? h('caption', null, p.caption) : null,
        h('thead', null, h('tr', null, cols.map(function (c) { return h('th', { key: c.key, scope: 'col', className: 'sc-al-' + (c.align || (c.kind && c.kind !== 'text' ? 'end' : 'start')) }, c.header); }))),
        h('tbody', null, rows.length ? rows.map(function (r, i) {
          return h('tr', { key: r.id || i }, cols.map(function (c, j) {
            var al = 'sc-al-' + (c.align || (c.kind && c.kind !== 'text' ? 'end' : 'start'));
            return j === 0 ? h('th', { key: c.key, scope: 'row', className: al }, cellFor(c.kind, r[c.key], r, c)) : h('td', { key: c.key, className: al }, cellFor(c.kind, r[c.key], r, c));
          }));
        }) : h('tr', { className: 'sc-table-empty' }, h('td', { colSpan: cols.length }, p.empty || '표시할 항목이 없어요')))));
  }

  /* ----- status ----- */
  /* one sentence, only when something is wrong; renders nothing without children */
  function StatusBar(p) {
    if (!p.children) return null;
    var tone = p.tone || 'warning';
    return h('div', { className: cx('sc-status', tone === 'info' && 'sc-status-info'), role: tone === 'error' ? 'alert' : 'status' },
      h(Icon, { name: tone === 'error' ? 'circle-x' : tone === 'info' ? 'info' : 'triangle-alert' }),
      h('span', { className: 'sc-status-text' }, p.children), p.action || null);
  }
  function EmptyState(p) {
    return h('div', { className: 'sc-empty' },
      h(Icon, { name: p.icon || 'info', size: 'lg' }),
      h('p', { className: 'sc-empty-title' }, p.title),
      p.children ? h('p', { className: 'sc-empty-text' }, p.children) : null,
      p.action ? h('div', { className: 'sc-empty-action' }, p.action) : null);
  }
  /* static placeholder, no shimmer */
  function Skeleton(p) {
    if (p.variant === 'block') return h('div', { className: 'sc-skel-block', 'aria-hidden': 'true', style: { height: tok(p.height || 'chart-height-sm', WIDTHS, 'Skeleton') } });
    var n = p.lines || 3, a = [];
    for (var i = 0; i < n; i++) a.push(h('div', { key: i, className: 'sc-skel-line' }));
    return h('div', { className: 'sc-skel', 'aria-hidden': 'true' }, a);
  }
  /* no repeating animation: a static ring plus words */
  function Spinner(p) {
    return h('span', { className: 'sc-spinner', role: 'status' }, h('span', { className: 'sc-spinner-ring', 'aria-hidden': 'true' }), p.label || '불러오는 중');
  }
  function Toast(p) {
    var tone = p.tone || 'info';
    return h('div', { className: cx('sc-toast', 'sc-toast-' + tone), role: tone === 'error' ? 'alert' : 'status' },
      h(Icon, { name: tone === 'error' ? 'circle-x' : tone === 'success' ? 'check' : 'info', size: 'md' }),
      h('div', { className: 'sc-toast-body' }, p.title ? h('div', { className: 'sc-toast-title' }, p.title) : null, p.children),
      p.onClose ? h(IconButton, { icon: 'x', label: '닫기', size: 'sm', onClick: p.onClose, tooltip: false }) : null);
  }

  /* ----- numbers ----- */
  function Money(p) {
    var f = moneyParts(p.value, p.currency || 'KRW', p.signed);
    return h('span', { className: cx('sc-num', 'sc-num-' + (p.size || 'md'), p.signed && f.ok && f.dir !== 'flat' && 'sc-num-signed-' + f.dir) }, Val(f));
  }
  function Percent(p) {
    var f = pctParts(p.value, p.signed);
    return h('span', { className: cx('sc-num', 'sc-num-' + (p.size || 'md'), p.signed && f.ok && f.dir !== 'flat' && 'sc-num-signed-' + f.dir) }, Val(f));
  }
  function Quantity(p) {
    var f = dec(p.value, { trim: true, kind: 'quantity' }); if (f.ok) f.text += '주';
    return h('span', { className: cx('sc-num', 'sc-num-' + (p.size || 'md')) }, Val(f));
  }
  var PROB = /^([1-9]|[1-9]\d)$/;
  function probOk(v) { return typeof v === 'string' && PROB.test(v); }
  /* whole percent 1–99 only */
  function Probability(p) {
    if (!probOk(p.value)) { report('probability', p.value, 'Probability'); return h('span', { className: cx('sc-num', 'sc-num-' + (p.size || 'md')) }, Val({ ok: false })); }
    return h('span', { className: cx('sc-num', 'sc-num-' + (p.size || 'md')) }, p.value + '%');
  }

  /* ----- Markdown: headings, lists, tables, code, links, quotes; images become links. No HTML. ----- */
  function safeHref(u) { return /^(https?:|mailto:)/i.test(u) ? u : null; }
  function inline(text, key) {
    var out = [], re = /(`[^`]+`)|(!?\[[^\]]*\]\([^)\s]+\))|(\*\*[^*]+\*\*)/g, last = 0, m, i = 0;
    while ((m = re.exec(text))) {
      if (m.index > last) out.push(text.slice(last, m.index));
      var t = m[0];
      if (m[1]) out.push(h('code', { key: key + i++ }, t.slice(1, -1)));
      else if (m[2]) {
        var mm = /^(!?)\[([^\]]*)\]\(([^)\s]+)\)$/.exec(t), href = safeHref(mm[3]), label = mm[1] ? (mm[2] || '이미지') + ' (이미지 링크)' : mm[2];
        out.push(href ? h('a', { key: key + i++, href: href, target: '_blank', rel: 'noreferrer' }, label) : label);
      } else out.push(h('strong', { key: key + i++ }, t.slice(2, -2)));
      last = re.lastIndex;
    }
    if (last < text.length) out.push(text.slice(last));
    return out;
  }
  function Markdown(p) {
    var lines = String(p.source || '').replace(/\r/g, '').split('\n'), blocks = [], i = 0, k = 0;
    while (i < lines.length) {
      var l = lines[i];
      if (/^\s*$/.test(l)) { i++; continue; }
      if (/^```/.test(l)) { var code = []; i++; while (i < lines.length && !/^```/.test(lines[i])) code.push(lines[i++]); i++; blocks.push(h('pre', { key: k++ }, h('code', null, code.join('\n')))); continue; }
      var hm = /^(#{1,3})\s+(.*)$/.exec(l);
      if (hm) { blocks.push(h('h' + hm[1].length, { key: k++ }, inline(hm[2], 'h' + k))); i++; continue; }
      if (/^\s*\|/.test(l) && i + 1 < lines.length && /^\s*\|?\s*:?-{3,}/.test(lines[i + 1])) {
        var cells = function (s) { return s.trim().replace(/^\||\|$/g, '').split('|').map(function (c) { return c.trim(); }); };
        var head = cells(l), body = []; i += 2;
        while (i < lines.length && /^\s*\|/.test(lines[i])) body.push(cells(lines[i++]));
        blocks.push(h('table', { key: k++ }, h('thead', null, h('tr', null, head.map(function (c, j) { return h('th', { key: j }, inline(c, 'th' + j)); }))),
          h('tbody', null, body.map(function (r, ri) { return h('tr', { key: ri }, r.map(function (c, j) { return h('td', { key: j }, inline(c, 'td' + ri + j)); })); }))));
        continue;
      }
      if (/^\s*([-*]|\d+\.)\s+/.test(l)) {
        var ordered = /^\s*\d+\./.test(l), items = [];
        while (i < lines.length && /^\s*([-*]|\d+\.)\s+/.test(lines[i])) items.push(lines[i++].replace(/^\s*([-*]|\d+\.)\s+/, ''));
        blocks.push(h(ordered ? 'ol' : 'ul', { key: k++ }, items.map(function (t, j) { return h('li', { key: j }, inline(t, 'li' + j)); })));
        continue;
      }
      if (/^>\s?/.test(l)) { var q = []; while (i < lines.length && /^>\s?/.test(lines[i])) q.push(lines[i++].replace(/^>\s?/, '')); blocks.push(h('blockquote', { key: k++ }, inline(q.join(' '), 'q'))); continue; }
      var para = [];
      while (i < lines.length && !/^\s*$/.test(lines[i]) && !/^(```|#{1,3}\s|>|\s*([-*]|\d+\.)\s|\s*\|)/.test(lines[i])) para.push(lines[i++]);
      if (!para.length) para.push(lines[i++]);
      blocks.push(h('p', { key: k++ }, inline(para.join(' '), 'p' + k)));
    }
    return h('div', { className: 'sc-md' }, blocks);
  }

  /* ----- ChatInput: Enter sends, Shift+Enter breaks the line ----- */
  function ChatInput(p) {
    var st = React.useState(''), value = p.value != null ? p.value : st[0];
    function set(v) { st[1](v); if (p.onChange) p.onChange(v); }
    function send() { var t = value.trim(); if (!t || p.disabled || p.streaming) return; if (p.onSend) p.onSend(t); set(''); }
    return h('div', { className: 'sc-chatin', 'data-disabled': p.disabled ? 'true' : undefined },
      h('textarea', {
        'aria-label': p.label || 'AI에게 보낼 말', placeholder: p.placeholder || '궁금한 걸 물어보세요', value: value, disabled: p.disabled, rows: 2,
        onChange: function (e) { set(e.target.value); },
        onKeyDown: function (e) { if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); send(); } }
      }),
      p.streaming
        ? h(IconButton, { icon: 'square', label: '답변 멈추기', onClick: p.onStop })
        : h(IconButton, { icon: 'send-horizontal', label: '보내기', onClick: send, disabled: p.disabled || !value.trim() }));
  }

  /* ================= charts =================
     Drawing only: strings are mapped to pixels. Axis labels reuse the core's own strings
     (the highest and lowest values given), so no tick is ever rounded here. */
  var HEIGHTS = ['chart-height-sm', 'chart-height', 'chart-height-lg'];
  function chartH(name, where) { if (HEIGHTS.indexOf(name) < 0) { report('token', name, where); name = 'chart-height'; } return cssLen(name) || 240; }
  function nums(arr, where) {
    var bad = false, out = (arr || []).map(function (v) { if (typeof v !== 'string' || !DEC.test(v)) { bad = true; return NaN; } return Number(v); });
    if (bad) report('chart', arr, where);
    return { ok: !bad, v: out };
  }
  function ChartEmpty(p) { return h('div', { className: 'sc-chart-empty', style: { height: p.height }, role: 'img', 'aria-label': p.text }, p.text); }
  function fmtAxis(s, p) { return p.format === 'percent' ? pctParts(s).text : p.format === 'number' ? dec(s).text : moneyParts(s, p.currency || 'KRW').text; }
  function extent(strs, vals) {
    var hi = 0, lo = 0;
    vals.forEach(function (v, i) { if (v > vals[hi]) hi = i; if (v < vals[lo]) lo = i; });
    return { hi: strs[hi], lo: strs[lo], max: vals[hi], min: vals[lo] };
  }
  var PAD = { t: 8, r: 8, b: 20, l: 72 };
  function Legend(p) {
    return h('ul', { className: 'sc-chart-legend' }, p.items.map(function (n, i) { return h('li', { key: n }, h('i', { className: 'sc-c' + (i + 1) }), n); }));
  }

  function LineChart(p) {
    var ref = React.useRef(null), w = useWidth(ref), H = chartH(p.height || 'chart-height', 'LineChart');
    var series = (p.series || []).slice(0, 6), labels = p.labels || [];
    var empty = !series.length || series.every(function (s) { return !s.values || s.values.length < 2; });
    var all = [], parsed = series.map(function (s) { var n = nums(s.values, 'LineChart'); all = all.concat(s.values || []); return n; });
    var bad = parsed.some(function (n) { return !n.ok; });
    var body;
    if (empty || bad || !w) body = w || empty || bad ? h(ChartEmpty, { height: H, text: bad ? '차트 값을 읽을 수 없어요' : (p.empty || '아직 그릴 데이터가 없어요') }) : null;
    else {
      var e = extent(all, all.map(Number)), span = e.max - e.min || 1, iw = w - PAD.l - PAD.r, ih = H - PAD.t - PAD.b;
      var X = function (i, n) { return PAD.l + (n < 2 ? 0 : i * iw / (n - 1)); }, Y = function (v) { return PAD.t + ih * (1 - (v - e.min) / span); };
      body = h('svg', { width: w, height: H, role: 'img', 'aria-label': p.label || '선 차트' },
        h('line', { className: 'sc-ax', x1: PAD.l, x2: w - PAD.r, y1: Y(e.max), y2: Y(e.max) }),
        h('line', { className: 'sc-ax-strong', x1: PAD.l, x2: w - PAD.r, y1: Y(e.min), y2: Y(e.min) }),
        h('text', { x: PAD.l - 8, y: Y(e.max) + 4, textAnchor: 'end' }, fmtAxis(e.hi, p)),
        h('text', { x: PAD.l - 8, y: Y(e.min) + 4, textAnchor: 'end' }, fmtAxis(e.lo, p)),
        labels.length ? h('text', { x: PAD.l, y: H - 4 }, labels[0]) : null,
        labels.length > 1 ? h('text', { x: w - PAD.r, y: H - 4, textAnchor: 'end' }, labels[labels.length - 1]) : null,
        p.area && parsed[0] ? h('path', { className: 'sc-area', d: 'M' + parsed[0].v.map(function (v, i) { return X(i, parsed[0].v.length) + ' ' + Y(v); }).join(' L') + ' L' + X(parsed[0].v.length - 1, parsed[0].v.length) + ' ' + Y(e.min) + ' L' + PAD.l + ' ' + Y(e.min) + ' Z' }) : null,
        parsed.map(function (n, si) { return h('polyline', { key: si, className: 'sc-line sc-s' + (si + 1), points: n.v.map(function (v, i) { return X(i, n.v.length) + ',' + Y(v); }).join(' ') }); }));
    }
    return h('div', { className: 'sc-chart', ref: ref }, body, series.length > 1 && !empty && !bad ? h(Legend, { items: series.map(function (s) { return s.name; }) }) : null);
  }
  function AreaChart(p) {
    return h(LineChart, { series: p.values ? [{ name: p.name || '값', values: p.values }] : [], labels: p.labels, height: p.height || 'chart-height-sm', format: p.format, currency: p.currency, empty: p.empty, label: p.label || '면적 차트', area: true });
  }

  function arc(cx0, cy0, r, a0, a1) {
    var x0 = cx0 + r * Math.sin(a0), y0 = cy0 - r * Math.cos(a0), x1 = cx0 + r * Math.sin(a1), y1 = cy0 - r * Math.cos(a1);
    return 'M' + x0 + ' ' + y0 + ' A' + r + ' ' + r + ' 0 ' + (a1 - a0 > Math.PI ? 1 : 0) + ' 1 ' + x1 + ' ' + y1;
  }
  function DonutChart(p) {
    var H = chartH(p.height || 'chart-height-sm', 'DonutChart'), items = p.items || [];
    if (!items.length) return h(ChartEmpty, { height: H, text: p.empty || '아직 그릴 데이터가 없어요' });
    var n = nums(items.map(function (it) { return it.weight; }), 'DonutChart');
    if (items.length > 6 || !n.ok) { if (items.length > 6) report('allocation', items.length, 'DonutChart'); return h(ChartEmpty, { height: H, text: items.length > 6 ? '항목이 6개를 넘어서 그리지 않았어요' : '차트 값을 읽을 수 없어요' }); }
    var total = n.v.reduce(function (a, b) { return a + b; }, 0) || 1, r = H / 2 - 12, c = H / 2, a = 0, gap = 0.02;
    var arcs = n.v.map(function (v, i) { var a0 = a, a1 = a + v / total * Math.PI * 2; a = a1; return h('path', { key: i, className: 'sc-line sc-s' + (i + 1), style: { strokeWidth: 16, strokeLinecap: 'butt' }, d: arc(c, c, r, a0 + gap, Math.max(a0 + gap, a1 - gap)) }); });
    return h('div', { className: 'sc-chart sc-donut-wrap' },
      h('svg', { width: H, height: H, role: 'img', 'aria-label': (p.label || '도넛 차트') + ': ' + items.map(function (it) { return it.label + ' ' + pct(it.weight); }).join(', ') }, arcs,
        p.centerValue ? h('text', { className: 'sc-center-v', x: c, y: c + 2, textAnchor: 'middle' }, p.centerValue) : null,
        p.centerLabel ? h('text', { className: 'sc-center-l', x: c, y: c + 20, textAnchor: 'middle' }, p.centerLabel) : null),
      h('ul', { className: 'sc-chart-legend', style: { flexDirection: 'column', margin: 0 } }, items.map(function (it, i) { return h('li', { key: it.label }, h('i', { className: 'sc-c' + (i + 1) }), it.label + ' ', h('b', { className: 'sc-num' }, pct(it.weight))); })));
  }

  /* squarified treemap; tile tint follows today's direction (up-soft / down-soft) */
  function squarify(items, x, y, w, hh) {
    var out = [], rest = items.slice(), total = rest.reduce(function (s, it) { return s + it.v; }, 0) || 1, scale = w * hh / total;
    rest = rest.map(function (it) { return Object.assign({}, it, { a: it.v * scale }); });
    function worst(row, side) { var s = 0, mx = 0, mn = Infinity; row.forEach(function (r) { s += r.a; mx = Math.max(mx, r.a); mn = Math.min(mn, r.a); }); return Math.max(side * side * mx / (s * s), s * s / (side * side * mn)); }
    while (rest.length) {
      var side = Math.min(w, hh), row = [rest.shift()];
      while (rest.length && worst(row.concat([rest[0]]), side) <= worst(row, side)) row.push(rest.shift());
      var s = row.reduce(function (t, r) { return t + r.a; }, 0);
      if (w >= hh) { var cw = s / hh, yy = y; row.forEach(function (r) { var rh = r.a / cw; out.push(Object.assign(r, { x: x, y: yy, w: cw, h: rh })); yy += rh; }); x += cw; w -= cw; }
      else { var ch = s / w, xx = x; row.forEach(function (r) { var rw = r.a / ch; out.push(Object.assign(r, { x: xx, y: y, w: rw, h: ch })); xx += rw; }); y += ch; hh -= ch; }
    }
    return out;
  }
  function Treemap(p) {
    var ref = React.useRef(null), w = useWidth(ref), H = chartH(p.height || 'chart-height', 'Treemap'), items = p.items || [];
    var n = nums(items.map(function (it) { return it.weight; }), 'Treemap');
    var body;
    if (!items.length || !n.ok) body = h(ChartEmpty, { height: H, text: !n.ok ? '차트 값을 읽을 수 없어요' : (p.empty || '아직 그릴 데이터가 없어요') });
    else if (w) {
      var tiles = squarify(items.map(function (it, i) { return { v: n.v[i], it: it }; }), 0, 0, w, H);
      body = h('svg', { width: w, height: H, role: 'img', 'aria-label': p.label || '트리맵' }, tiles.map(function (t, i) {
        var r = t.it.rate !== undefined ? pctParts(t.it.rate, true) : null, d = r && r.ok ? r.dir : 'flat';
        var big = t.w > 64 && t.h > 40;
        return h('g', { key: i },
          h('rect', { className: 'sc-tile sc-tile-' + d, x: t.x, y: t.y, width: t.w, height: t.h, rx: 4 }),
          big ? h('text', { className: 'sc-tile-name', x: t.x + 8, y: t.y + 20 }, t.it.label) : null,
          big && r ? h('text', { className: 'sc-tile-' + d + '-t', x: t.x + 8, y: t.y + 38 }, (d === 'up' ? '▲ ' : d === 'down' ? '▼ ' : '– ') + r.text) : null,
          h('title', null, t.it.label + (r ? ' ' + r.text : '')));
      }));
    }
    return h('div', { className: 'sc-chart', ref: ref }, body);
  }

  function CandlestickChart(p) {
    var ref = React.useRef(null), w = useWidth(ref), H = chartH(p.height || 'chart-height-lg', 'CandlestickChart'), cs = p.candles || [];
    var flat = []; cs.forEach(function (c) { flat.push(c.open, c.high, c.low, c.close); });
    var n = nums(flat, 'CandlestickChart'), body;
    if (cs.length < 2 || !n.ok) body = h(ChartEmpty, { height: H, text: !n.ok ? '차트 값을 읽을 수 없어요' : (p.empty || '아직 그릴 시세가 없어요') });
    else if (w) {
      var highs = cs.map(function (c) { return c.high; }), lows = cs.map(function (c) { return c.low; });
      var eh = extent(highs, highs.map(Number)), el = extent(lows, lows.map(Number)), max = eh.max, min = el.min, span = max - min || 1;
      var iw = w - PAD.l - PAD.r, ih = H - PAD.t - PAD.b, step = iw / cs.length, bw = Math.max(1, Math.min(12, step * 0.6));
      var Y = function (v) { return PAD.t + ih * (1 - (v - min) / span); };
      body = h('svg', { width: w, height: H, role: 'img', 'aria-label': p.label || '캔들 차트' },
        h('line', { className: 'sc-ax', x1: PAD.l, x2: w - PAD.r, y1: Y(max), y2: Y(max) }),
        h('line', { className: 'sc-ax', x1: PAD.l, x2: w - PAD.r, y1: Y(min), y2: Y(min) }),
        h('text', { x: PAD.l - 8, y: Y(max) + 4, textAnchor: 'end' }, fmtAxis(eh.hi, p)),
        h('text', { x: PAD.l - 8, y: Y(min) + 4, textAnchor: 'end' }, fmtAxis(el.lo, p)),
        h('text', { x: PAD.l, y: H - 4 }, cs[0].label),
        h('text', { x: w - PAD.r, y: H - 4, textAnchor: 'end' }, cs[cs.length - 1].label),
        cs.map(function (c, i) {
          var o = Number(c.open), cl = Number(c.close), d = cl > o ? 'up' : cl < o ? 'down' : 'flatc', x = PAD.l + step * i + step / 2;
          var top = Y(Math.max(o, cl)), bh = Math.max(1, Math.abs(Y(o) - Y(cl)));
          return h('g', { key: i, className: 'sc-' + d },
            h('line', { x1: x, x2: x, y1: Y(Number(c.high)), y2: Y(Number(c.low)), strokeWidth: 1 }),
            h('rect', { x: x - bw / 2, y: top, width: bw, height: bh, strokeWidth: 0 }));
        }));
    }
    return h('div', { className: 'sc-chart', ref: ref }, body);
  }

  /* predicted probability (x) vs actual hit rate (y), 0–100; points the core has not judged are hollow */
  function ReliabilityChart(p) {
    var H = chartH(p.height || 'chart-height', 'ReliabilityChart'), pts = p.points || [];
    if (!pts.length) return h(ChartEmpty, { height: H, text: p.empty || '아직 판정된 예측이 없어요' });
    var n = nums([].concat(pts.map(function (q) { return q.predicted; }), pts.map(function (q) { return q.actual; })), 'ReliabilityChart');
    if (!n.ok) return h(ChartEmpty, { height: H, text: '차트 값을 읽을 수 없어요' });
    var L = 44, B = 32, T = 8, R = 8, S = H - T - B, W = S + L + R;
    var X = function (v) { return L + S * v / 100; }, Y = function (v) { return T + S * (1 - v / 100); };
    return h('div', { className: 'sc-chart' },
      h('svg', { width: W, height: H, role: 'img', 'aria-label': p.label || '예측 확률 대비 실제 적중률' },
        h('rect', { x: L, y: T, width: S, height: S, fill: 'none', className: 'sc-ax' }),
        h('line', { className: 'sc-diag', x1: X(0), y1: Y(0), x2: X(100), y2: Y(100) }),
        ['0', '50', '100'].map(function (t) { return h('g', { key: t }, h('text', { x: X(Number(t)), y: T + S + 14, textAnchor: 'middle' }, t + '%'), h('text', { x: L - 6, y: Y(Number(t)) + 4, textAnchor: 'end' }, t + '%')); }),
        h('text', { x: L + S / 2, y: H - 2, textAnchor: 'middle' }, '예측 확률'),
        h('text', { x: 10, y: T + S / 2, textAnchor: 'middle', transform: 'rotate(-90 10 ' + (T + S / 2) + ')' }, '실제 적중률'),
        pts.map(function (q, i) {
          var few = q.judged !== true;
          return h('circle', { key: i, className: few ? 'sc-pt-hollow' : 'sc-pt', cx: X(Number(q.predicted)), cy: Y(Number(q.actual)), r: 5 }, h('title', null, '예측 ' + q.predicted + '% · 실제 ' + q.actual + '% · ' + q.count + '건' + (few ? ' (판정 보류)' : '')));
        })),
      h('ul', { className: 'sc-chart-legend' }, h('li', null, h('i', { className: 'sc-c1' }), '판정됨'), h('li', null, h('i', { style: { boxShadow: 'inset 0 0 0 1.5px var(--chart-1)' } }), '판정 보류'), h('li', null, '점선 = 예측과 실제가 같은 선')));
  }

  /* ================= product ================= */
  function probPos(v) { return Math.max(0, Math.min(100, Number(v))) + '%'; }
  /* probability (head line), base rate and the two thresholds on one 0–100 track. Neutral colours only.
     Labels are laid out in rows so they never overlap, even when every mark sits in the left half. */
  function labelWidth(t) { var w = 0; for (var i = 0; i < t.length; i++) w += t.charCodeAt(i) > 0x2000 ? 11 : 6.5; return w; }
  function ProbabilityMeter(p) {
    var ref = React.useRef(null), tw = useWidth(ref);
    var vals = [p.probability, p.base, p.buyThreshold, p.sellThreshold];
    if (!vals.every(function (v) { return v === undefined || probOk(v); })) { report('probability', vals, 'ProbabilityMeter'); return h('div', { className: 'sc-meter' }, Val({ ok: false })); }
    var labs = [];
    if (p.sellThreshold) labs.push({ k: 's', v: Number(p.sellThreshold), t: '매도 기준 ' + p.sellThreshold + '%' });
    if (p.base) labs.push({ k: 'm', v: Number(p.base), t: '기준 ' + p.base + '%', base: true });
    if (p.buyThreshold) labs.push({ k: 'b', v: Number(p.buyThreshold), t: '매수 기준 ' + p.buyThreshold + '%' });
    labs.sort(function (x, y) { return x.v - y.v; });
    var rows = { bot: [], top: [], bot2: [] }, order = ['bot', 'top', 'bot2'], GAP = 8;
    labs.forEach(function (l) {
      l.w = labelWidth(l.t); l.x = tw ? Math.max(0, Math.min(tw - l.w, tw * l.v / 100 - l.w / 2)) : null;
      var pref = l.base ? ['top', 'bot', 'bot2'] : order;
      for (var i = 0; i < pref.length; i++) {
        var clash = tw && rows[pref[i]].some(function (o) { return l.x < o.x + o.w + GAP && o.x < l.x + l.w + GAP; });
        if (!clash) { rows[pref[i]].push(l); l.row = pref[i]; break; }
      }
      if (!l.row) { rows.bot2.push(l); l.row = 'bot2'; }
    });
    var marks = labs.map(function (l) {
      return [h('span', { key: l.k, className: cx('sc-meter-mark', l.base && 'sc-meter-mark-base'), style: { left: l.v + '%' } }),
        h('span', { key: l.k + 'l', className: 'sc-meter-lab sc-meter-lab-' + l.row, style: tw ? { left: l.x, transform: 'none' } : { left: l.v + '%' } }, l.t)];
    });
    var label = (p.probabilityLabel || '확률') + ' ' + p.probability + '%' + (p.base ? ', 기준 ' + p.base + '%' : '') + (p.buyThreshold ? ', 매수 기준 ' + p.buyThreshold + '%' : '') + (p.sellThreshold ? ', 매도 기준 ' + p.sellThreshold + '%' : '');
    return h('div', { className: 'sc-meter', role: 'img', 'aria-label': label },
      h('div', { className: 'sc-meter-head' }, h('span', null, p.probabilityLabel || '확률'), h('b', null, p.probability + '%')),
      h('div', { ref: ref, className: cx('sc-meter-track', rows.top.length && 'sc-meter-has-top', rows.bot2.length && 'sc-meter-has-bot2') },
        h('span', { className: 'sc-meter-fill', style: { width: probPos(p.probability) } }),
        marks));
  }

  function Fact(label, node) { return h('div', { key: label }, h('dt', null, label), h('dd', null, node)); }
  function hitText(hist) {
    if (!hist) return null;
    if (hist.judged !== true) return h('span', { className: 'sc-pending' }, '판정 보류 (' + hist.count + '건)');
    return h('span', null, h(Probability, { value: hist.hitRate }), ' (' + hist.count + '건)');
  }
  /* states: default · ordersOff · zeroQty · externalOnly · recordOnly · pending · filled · rejected */
  function SignalCard(p) {
    var st = p.state || 'default', side = p.side === 'sell' ? 'sell' : 'buy', word = side === 'buy' ? '매수' : '매도', c = p.currency || 'KRW', s = p.stock || {};
    function priceCell(label, value, rate, sub) {
      return h('div', { className: 'sc-sig-price' }, h('span', { className: 'sc-sig-price-l' }, label),
        value, rate !== undefined ? h(PriceChange, { rate: rate, size: 'sm' }) : sub ? h('span', { className: 'sc-sig-price-sub' }, sub) : null);
    }
    var prices = h('div', { className: 'sc-sig-prices' },
      priceCell('현재가', h(Money, { value: p.price, currency: c })),
      priceCell('목표가', h(Money, { value: p.targetPrice, currency: c }), p.targetRate),
      priceCell('손절가', h(Money, { value: p.stopPrice, currency: c }), p.stopRate),
      priceCell('기간', h('span', { className: 'sc-num sc-num-md' }, p.horizon || '—'), undefined, p.horizonEnd ? p.horizonEnd + '까지' : null));
    var facts = h('dl', { className: 'sc-sig-dl' },
      Fact('수량', h(Quantity, { value: p.quantity })),
      Fact('금액', h(Money, { value: p.amount, currency: c })),
      Fact('수수료·세금', h(Money, { value: p.costs, currency: c })),
      Fact('주문 후 비중', h(Percent, { value: p.weightAfter })),
      Fact('과거 적중률', hitText(p.history)));
    var foot;
    if (st === 'default' || st === 'externalOnly') foot = [h('span', { key: 't', className: 'sc-sig-foot-text' }, '누르면 macOS 확인 창이 떠요. Touch ID로 승인해야 주문이 나가요.'),
      h(Button, { key: 'b', variant: side, onClick: p.onOrder }, h(Icon, { name: 'fingerprint' }), word + ' 주문 넣기')];
    else if (st === 'ordersOff') foot = h('span', { className: 'sc-sig-foot-text' }, '주문 기능이 꺼져 있어서 신호만 보여줘요. 설정에서 켤 수 있어요.');
    else if (st === 'zeroQty') foot = h('span', { className: 'sc-sig-foot-text' }, p.reason || '살 수 있는 수량이 0주예요.');
    else if (st === 'recordOnly') foot = h('span', { className: 'sc-sig-foot-text' }, '지금은 기록만 하는 기간이라 주문 버튼이 없어요. 판정된 추정이 30건 쌓이면 자동으로 끝나요.');
    else if (st === 'pending') foot = h(Spinner, { label: 'Touch ID 승인을 기다리는 중' });
    else if (st === 'filled') foot = h('span', { className: 'sc-sig-foot-text sc-sig-done' }, h(Icon, { name: 'check' }), ' ' + word + ' 체결됐어요' + (p.doneAt ? ' · ' + p.doneAt : ''));
    else if (st === 'rejected') foot = h('div', { style: { flex: 1 } }, h(Callout, { tone: 'error', title: word + ' 주문이 나가지 않았어요', action: p.onRetry ? h(Button, { size: 'sm', onClick: p.onRetry }, '다시 시도') : null }, p.reason || '토스증권이 주문을 거절했어요. 정정·취소는 토스증권 앱에서 해주세요.'));
    return h('article', { className: 'sc-sig', 'aria-label': (s.name || '') + ' ' + word + ' 신호' },
      h('header', { className: 'sc-sig-head' },
        h(Badge, { tone: side === 'buy' ? 'up' : 'down' }, word + ' 신호'),
        h('div', { className: 'sc-sig-title' }, h('b', null, s.name), h('span', { className: 'sc-sig-sub' }, [s.ticker, s.market].filter(Boolean).join(' · '))),
        p.time ? h('span', { className: 'sc-feed-time' }, p.time) : null),
      h('section', { className: 'sc-sig-zone sc-sig-facts', 'aria-label': '앱이 계산한 사실' },
        h('div', { className: 'sc-sig-zone-label' }, h(Icon, { name: 'target' }), '앱이 계산한 사실'),
        prices,
        h(ProbabilityMeter, { probability: p.adjustedProbability, probabilityLabel: '기간 안에 손절가보다 목표가에 먼저 닿을 확률', base: p.baseProbability, buyThreshold: p.buyThreshold, sellThreshold: p.sellThreshold }),
        h(Inline, { gap: 'space-6', wrap: true },
          h(Text, { variant: 'body-sm', tone: 'muted', as: 'span' }, 'AI가 낸 확률 ', h('b', { className: 'sc-num' }, Val(probOk(p.aiProbability) ? { ok: true, text: p.aiProbability + '%' } : { ok: false }))),
          h(Text, { variant: 'body-sm', tone: 'muted', as: 'span' }, '적중 기록 반영 ', h('b', { className: 'sc-num' }, Val(probOk(p.adjustedProbability) ? { ok: true, text: p.adjustedProbability + '%' } : { ok: false }))),
          h(Text, { variant: 'body-sm', tone: 'muted', as: 'span' }, word + ' 기준 ', h('b', { className: 'sc-num' }, Val(probOk(side === 'buy' ? p.buyThreshold : p.sellThreshold) ? { ok: true, text: (side === 'buy' ? p.buyThreshold : p.sellThreshold) + '%' } : { ok: false })))),
        facts),
      h('section', { className: 'sc-sig-zone sc-sig-ai', 'aria-label': 'AI 근거' },
        h('div', { className: 'sc-sig-zone-label' }, h(Icon, { name: 'message-square' }), 'AI 근거'),
        st === 'externalOnly' ? h(Callout, { tone: 'warning', title: '근거가 외부 출처뿐이에요' }, '내 보유·시세 데이터로 확인된 근거가 없어요. 출처를 직접 읽어보고 판단해주세요.') : null,
        h(Markdown, { source: p.reasons || '' }),
        p.sources && p.sources.length ? h('div', { className: 'sc-msg-sources' }, p.sources.map(function (x, i) { return x.href ? h('a', { key: i, href: x.href, target: '_blank', rel: 'noreferrer' }, x.label) : h('span', { key: i }, x.label); })) : h('div', { className: 'sc-msg-sources' }, h('span', { className: 'sc-tone-alert' }, '출처 없음'))),
      h('footer', { className: 'sc-sig-foot' }, foot));
  }

  function FeedCard(icon, p, kids) {
    return h('article', { className: 'sc-feed-card' },
      h('header', { className: 'sc-feed-head' }, h(Icon, { name: icon, size: 'md' }), h(Heading, { level: 3 }, p.title), p.time ? h('span', { className: 'sc-feed-time' }, p.time) : null),
      kids);
  }
  /* kind: morning | close. figures: [{label, value(node)}] */
  function BriefingCard(p) {
    return FeedCard(p.kind === 'close' ? 'sunset' : 'sunrise', { title: p.title || (p.kind === 'close' ? '마감 리뷰' : '아침 브리핑'), time: p.time }, [
      p.figures && p.figures.length ? h('div', { key: 'f', className: 'sc-feed-figs' }, p.figures.map(function (f) { return h('div', { key: f.label, className: 'sc-feed-fig' }, h('span', null, f.label), h('span', null, f.value)); })) : null,
      h('div', { key: 'b', className: 'sc-feed-body' }, h(Markdown, { source: p.summary || '' })),
      p.sources && p.sources.length ? h('div', { key: 's', className: 'sc-msg-sources' }, p.sources.map(function (x, i) { return x.href ? h('a', { key: i, href: x.href, target: '_blank', rel: 'noreferrer' }, x.label) : h('span', { key: i }, x.label); })) : null]);
  }
  function DisclosureCard(p) {
    var s = p.stock || {};
    return FeedCard('file-text', { title: s.name + ' · ' + p.title, time: p.time }, [
      h(Inline, { key: 'm', gap: 'space-2', wrap: true }, h(Badge, null, p.kind || '공시'), p.held ? h(Badge, { tone: 'primary' }, '보유 종목') : null),
      p.summary ? h('div', { key: 'b', className: 'sc-feed-body' }, h(Markdown, { source: p.summary })) : null,
      p.href ? h('div', { key: 'l', className: 'sc-msg-sources' }, h('a', { href: p.href, target: '_blank', rel: 'noreferrer' }, '공시 원문 보기')) : null]);
  }

  /* compact = 2-letter rail item used when the list is collapsed */
  function StockListItem(p) {
    var r = p.rate !== undefined ? pctParts(p.rate, true) : null, d = r && r.ok ? r.dir : 'flat';
    var common = { type: 'button', className: cx('sc-sitem', p.compact && 'sc-sitem-compact'), 'aria-current': p.selected ? 'true' : undefined, onClick: p.onSelect };
    if (p.compact) {
      return h(Tooltip, { label: p.name + (r ? ' ' + r.text : ''), side: 'right' },
        h('button', Object.assign({ 'aria-label': p.name + (r ? ', ' + (WORD[d] || '값 오류') + ' ' + r.text : '') }, common),
          h('span', { className: 'sc-sitem-abbr' }, Array.from(p.name || '').slice(0, 2).join('')),
          h('span', { className: cx('sc-sitem-arrow', 'sc-chg-' + (r && r.ok ? d : 'invalid')) }, r && r.ok ? ARROW[d] : '—')));
    }
    return h('button', common,
      h('span', { className: 'sc-sitem-name' }, h('b', null, p.name), h('span', null, p.ticker)),
      p.rate !== undefined ? h(PriceChange, { rate: p.rate, size: 'sm' }) : null);
  }
  /* held and watched stocks with today's change. Follows the AppShell list mode. */
  function StockList(p) {
    var mode = React.useContext(ListModeCtx), shell = useAppShell(), compact = mode === 'compact';
    function group(title, items) {
      if (!items || !items.length) return null;
      return h('div', { key: title },
        h('div', { className: 'sc-slist-label', 'aria-hidden': compact ? 'true' : undefined }, compact ? title.slice(0, 2) : title),
        h('ul', { className: 'sc-slist-group', 'aria-label': title }, items.map(function (it) {
          return h('li', { key: it.id || it.ticker }, h(StockListItem, Object.assign({}, it, { compact: compact, selected: (it.id || it.ticker) === p.selectedId, onSelect: function () { if (p.onSelect) p.onSelect(it); if (mode === 'overlay') shell.setListOpen(false); } })));
        })));
    }
    var empty = !(p.held && p.held.length) && !(p.watch && p.watch.length);
    return h('div', { className: cx('sc-slist', compact && 'sc-slist-compact') },
      h('div', { className: 'sc-slist-head' },
        compact ? null : h(Heading, { level: 2 }, '종목'),
        mode === 'compact' ? h(IconButton, { icon: 'panel-left-open', label: '종목 목록 펼치기', expanded: false, tooltipSide: 'right', onClick: function () { shell.setListOpen(true); } }) : null,
        mode === 'overlay' ? h(IconButton, { icon: 'panel-left-close', label: '종목 목록 접기', expanded: true, onClick: function () { shell.setListOpen(false); } }) : null),
      h('div', { className: 'sc-slist-scroll' },
        p.loading ? h('div', { style: { padding: 'var(--space-4)' } }, h(Skeleton, { lines: 6 })) :
          empty ? (compact ? null : h(EmptyState, { icon: 'star', title: '아직 종목이 없어요' }, '토스에 연결하면 보유 종목이 여기 보여요.')) :
            [group('보유', p.held), group('관심', p.watch)]));
  }

  /* rows: {band, predicted, actual, count, judged}; judged comes from the core */
  function ScorecardTable(p) {
    var rows = p.rows || [];
    return h('div', { className: 'sc-table-wrap' },
      h('table', { className: 'sc-table' },
        p.caption ? h('caption', null, p.caption) : null,
        h('thead', null, h('tr', null, ['확률 구간', '평균 예측', '실제 적중률', '건수'].map(function (x) { return h('th', { key: x, scope: 'col' }, x); }))),
        h('tbody', null, rows.length ? rows.map(function (r) {
          var few = r.judged !== true;
          return h('tr', { key: r.band },
            h('th', { scope: 'row' }, r.band),
            h('td', null, h(Probability, { value: r.predicted })),
            h('td', null, few ? h('span', { className: 'sc-pending' }, '판정 보류') : h(Probability, { value: r.actual })),
            h('td', null, Val(dec(r.count, { kind: 'count' })) , '건'));
        }) : h('tr', { className: 'sc-table-empty' }, h('td', { colSpan: 4 }, p.empty || '아직 판정된 신호가 없어요')))));
  }

  /* status: idle · looking (조회 중 한 줄) · streaming · error */
  function ChatPanel(p) {
    var shell = useAppShell(), logRef = React.useRef(null), st = p.status || 'idle', msgs = p.messages || [];
    React.useEffect(function () { var el = logRef.current; if (el) el.scrollTop = el.scrollHeight; }, [msgs.length, st]);
    return h('div', { className: 'sc-chat' },
      h('div', { className: 'sc-chat-head' }, h(Heading, { level: 2 }, p.title || 'AI 비서'),
        shell.chatMode !== 'none' ? h(IconButton, { icon: 'panel-right-close', label: '채팅 닫기', onClick: function () { shell.setChatOpen(false); } }) : null),
      h('div', { className: 'sc-chat-log', ref: logRef, role: 'log', 'aria-live': 'polite', tabIndex: 0 },
        msgs.length ? msgs.map(function (m, i) {
          var last = i === msgs.length - 1;
          return h(ChatMessage, { key: i, role: m.role, activity: m.activity, sources: m.sources, note: m.note, streaming: last && st === 'streaming' && m.role === 'assistant' }, m.content);
        }) : h(EmptyState, { icon: 'message-square', title: '무엇이든 물어보세요' }, '보유 종목, 신호의 근거, 오늘 시장을 물어볼 수 있어요. 숫자는 앱이 계산한 값만 써요.'),
        st === 'looking' ? h('div', { className: 'sc-msg-activity' }, h(Spinner, { label: p.activity || '보유 종목을 확인하는 중' })) : null),
      h('div', { className: 'sc-chat-foot' },
        st === 'error' ? h(Callout, { tone: 'error', title: '답변을 받지 못했어요', action: p.onRetry ? h(Button, { size: 'sm', onClick: p.onRetry }, '다시 시도') : null }, p.error || 'AI 연결이 끊겼어요. 잠시 후 다시 시도해주세요.') : null,
        h(ChatInput, { onSend: p.onSend, onStop: p.onStop, streaming: st === 'streaming' || st === 'looking', disabled: p.disabled })));
  }

  var api = {
    Button: Button, PriceChange: PriceChange, Badge: Badge, Metric: Metric, Sparkline: Sparkline, AllocationBar: AllocationBar, HoldingsTable: HoldingsTable, SegmentedControl: SegmentedControl, ChatMessage: ChatMessage, Callout: Callout,
    AppShell: AppShell, Page: Page, Section: Section, Grid: Grid, GridItem: GridItem, Stack: Stack, Inline: Inline, Split: Split, Card: Card, Toolbar: Toolbar, ScrollArea: ScrollArea, Divider: Divider,
    Text: Text, Heading: Heading, Icon: Icon, IconButton: IconButton, TextField: TextField, Select: Select, Switch: Switch, Checkbox: Checkbox,
    Disclosure: Disclosure, Dialog: Dialog, Popover: Popover, Menu: Menu, Tooltip: Tooltip, Table: Table, StatusBar: StatusBar, EmptyState: EmptyState, Skeleton: Skeleton, Spinner: Spinner, Toast: Toast,
    Money: Money, Percent: Percent, Quantity: Quantity, Probability: Probability, Markdown: Markdown, ChatInput: ChatInput,
    LineChart: LineChart, AreaChart: AreaChart, DonutChart: DonutChart, Treemap: Treemap, CandlestickChart: CandlestickChart, ReliabilityChart: ReliabilityChart,
    SignalCard: SignalCard, ProbabilityMeter: ProbabilityMeter, BriefingCard: BriefingCard, DisclosureCard: DisclosureCard, StockListItem: StockListItem, StockList: StockList, ScorecardTable: ScorecardTable, ChatPanel: ChatPanel,
    useAppShell: useAppShell, icons: Object.keys(ICONS),
    format: { money: money, pct: pct, qty: qty, num: num, krwShort: krwShort, dir: dir, isValid: function (x) { return typeof x === 'string' && DEC.test(x); }, INVALID: INVALID },
    onFormatError: function (fn) { listeners.push(fn); return function () { listeners = listeners.filter(function (f) { return f !== fn; }); }; }
  };
  window.StockCopilot = Object.assign(window.StockCopilot || {}, api);
})();
