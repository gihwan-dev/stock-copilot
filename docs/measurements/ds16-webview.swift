import AppKit
import WebKit

let args = CommandLine.arguments
guard args.count >= 3 else {
    print("usage: ds16-webview <cases.json> <snapshot.png>")
    exit(1)
}
let casesJSON = (try? String(contentsOfFile: args[1], encoding: .utf8)) ?? "[]"
let snapshotPath = args[2]
let fontStack = ProcessInfo.processInfo.environment["FONT_STACK"] ?? "\"IBM Plex Sans KR\", \"Apple SD Gothic Neo\", \"Malgun Gothic\", system-ui, sans-serif"

let page = """
<!doctype html><html lang="ko"><head><meta charset="utf-8"><style>
:root { --font-sans: \(fontStack); }
body { font-family: var(--font-sans); margin: 16px; background: #fff; color: #111; }
.row { display: flex; gap: 16px; align-items: baseline; height: 24px; }
.label { width: 260px; font-size: 12px; color: #666; }
.num { font-variant-numeric: tabular-nums; white-space: nowrap; font-size: 14px; font-weight: 500; }
.lg { font-size: 22px; font-weight: 600; }
textarea { width: 300px; height: 40px; font-family: var(--font-sans); }
</style></head><body><div id="rows"></div><textarea id="chat"></textarea><script>
const cases = \(casesJSON);
const rows = document.getElementById('rows');
for (const c of cases) {
  const r = document.createElement('div'); r.className = 'row';
  const l = document.createElement('span'); l.className = 'label'; l.textContent = c.label;
  const n = document.createElement('span'); n.className = 'num'; n.textContent = c.display.text;
  const g = document.createElement('span'); g.className = 'num lg'; g.textContent = c.display.text;
  r.append(l, n, g); rows.append(r);
}
window.events = [];
const t = document.getElementById('chat');
for (const type of ['compositionstart','compositionupdate','compositionend','beforeinput','input','keydown','keyup']) {
  t.addEventListener(type, e => window.events.push({type, data: e.data ?? null, key: e.key ?? null, keyCode: e.keyCode ?? null, isComposing: e.isComposing ?? null, value: t.value}));
}
function widthOf(text, cls) {
  const s = document.createElement('span'); s.className = cls; s.textContent = text;
  document.body.append(s); const w = s.getBoundingClientRect().width; s.remove(); return w;
}
function fontPresent(family) {
  const c = document.createElement('canvas').getContext('2d');
  const probe = '0123456789가나다한글';
  c.font = '20px monospace'; const base = c.measureText(probe).width;
  c.font = '20px "' + family + '", monospace'; const w = c.measureText(probe).width;
  return Math.abs(w - base) > 0.01;
}
window.report = () => {
  const digits = {};
  for (const cls of ['num', 'num lg']) digits[cls] = [...'0123456789'].map(d => +widthOf(d, cls).toFixed(3));
  const texts = cases.map(c => c.display.text);
  return JSON.stringify({
    fonts: { 'IBM Plex Sans KR': fontPresent('IBM Plex Sans KR'), 'Apple SD Gothic Neo': fontPresent('Apple SD Gothic Neo') },
    digitWidths: digits,
    tabularEqual: Object.fromEntries(Object.entries(digits).map(([k, v]) => [k, Math.max(...v) - Math.min(...v) < 0.01])),
    sameLengthEqualWidth: { num: Math.abs(widthOf('1,111,111원','num') - widthOf('8,888,888원','num')) < 0.01, lg: Math.abs(widthOf('1,111,111원','num lg') - widthOf('8,888,888원','num lg')) < 0.01 },
    minusSign: { u2212Count: texts.join('').split('\\u2212').length - 1, hyphenMinusCount: texts.join('').split('-').length - 1, u2212Width: +widthOf('\\u2212','num').toFixed(3), plusWidth: +widthOf('+','num').toFixed(3), tofuWidth: +widthOf('\\u{10FFFD}','num').toFixed(3) },
    rendered: [...document.querySelectorAll('.row')].map(r => ({ label: r.children[0].textContent, text: r.children[1].textContent, width: +r.children[1].getBoundingClientRect().width.toFixed(2), height: +r.children[1].getBoundingClientRect().height.toFixed(2) })),
  });
};
</script></body></html>
"""

final class Runner: NSObject, WKNavigationDelegate {
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 900, height: 480), styleMask: [.titled], backing: .buffered, defer: false)
    let webView = WKWebView(frame: NSRect(x: 0, y: 0, width: 900, height: 480))

    func start() {
        window.contentView = webView
        window.setFrameOrigin(NSPoint(x: -4000, y: -4000))
        window.orderFrontRegardless()
        webView.navigationDelegate = self
        webView.loadHTMLString(page, baseURL: nil)
    }

    func eval(_ js: String, _ done: @escaping (Any?) -> Void) {
        webView.evaluateJavaScript(js) { value, error in
            if let error { print("js-error:", error) }
            done(value)
        }
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { self.measure() }
    }

    func measure() {
        eval("window.report()") { value in
            print("REPORT", value as? String ?? "null")
            let config = WKSnapshotConfiguration()
            self.webView.takeSnapshot(with: config) { image, _ in
                if let image, let tiff = image.tiffRepresentation, let rep = NSBitmapImageRep(data: tiff), let png = rep.representation(using: .png, properties: [:]) {
                    try? png.write(to: URL(fileURLWithPath: snapshotPath))
                    print("SNAPSHOT", snapshotPath)
                }
                self.compose()
            }
        }
    }

    func compose() {
        window.makeKey()
        window.makeFirstResponder(webView)
        eval("document.getElementById('chat').focus(); document.activeElement.id") { focused in
            print("FOCUSED", focused as? String ?? "null")
            let client = self.webView as AnyObject
            let none = NSRange(location: NSNotFound, length: 0)
            let steps: [(String, Bool)] = [("ㅎ", false), ("하", false), ("한", false), ("한", true)]
            var index = 0
            func next() {
                guard index < steps.count else {
                    self.enterDuringComposition()
                    return
                }
                let (textValue, commit) = steps[index]
                index += 1
                if commit {
                    _ = client.perform(NSSelectorFromString("insertText:replacementRange:"), with: textValue, with: NSValue(range: none))
                } else {
                    let sel = NSSelectorFromString("setMarkedText:selectedRange:replacementRange:")
                    typealias Fn = @convention(c) (AnyObject, Selector, Any, NSRange, NSRange) -> Void
                    let imp = unsafeBitCast(self.webView.method(for: sel), to: Fn.self)
                    imp(self.webView, sel, textValue, NSRange(location: (textValue as NSString).length, length: 0), none)
                }
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.2, execute: next)
            }
            next()
        }
    }

    func enterDuringComposition() {
        eval("JSON.stringify(window.events.splice(0))") { value in
            print("COMPOSE_EVENTS", value as? String ?? "null")
            let sel = NSSelectorFromString("setMarkedText:selectedRange:replacementRange:")
            typealias Fn = @convention(c) (AnyObject, Selector, Any, NSRange, NSRange) -> Void
            let imp = unsafeBitCast(self.webView.method(for: sel), to: Fn.self)
            imp(self.webView, sel, "글", NSRange(location: 1, length: 0), NSRange(location: NSNotFound, length: 0))
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
                let down = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: self.window.windowNumber, context: nil, characters: "\r", charactersIgnoringModifiers: "\r", isARepeat: false, keyCode: 36)!
                let up = NSEvent.keyEvent(with: .keyUp, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: self.window.windowNumber, context: nil, characters: "\r", charactersIgnoringModifiers: "\r", isARepeat: false, keyCode: 36)!
                self.webView.keyDown(with: down)
                self.webView.keyUp(with: up)
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) {
                    self.eval("JSON.stringify(window.events.splice(0))") { value in
                        print("ENTER_EVENTS", value as? String ?? "null")
                        NSApp.terminate(nil)
                    }
                }
            }
        }
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let runner = Runner()
runner.start()
DispatchQueue.main.asyncAfter(deadline: .now() + 30) { print("timeout"); exit(4) }
app.run()
