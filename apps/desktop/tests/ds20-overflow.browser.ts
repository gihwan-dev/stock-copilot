import { inspectOverflow } from "../src/shared/ds/testing/overflow";
import { stressFixtures } from "../src/shared/ds/testing/stress";

const makeDocument = async (
  body: string,
  htmlAttributes = "",
): Promise<Document> => {
  const frame = document.createElement("iframe");
  frame.width = "640";
  frame.height = "480";
  const loaded = new Promise<void>((resolve) => {
    frame.addEventListener("load", () => resolve(), { once: true });
  });
  frame.srcdoc = `<html ${htmlAttributes}><head><style>html,body{margin:0;width:100%;height:100%}*{box-sizing:border-box}body{font:16px sans-serif}</style></head><body>${body}</body></html>`;
  document.body.append(frame);
  await loaded;
  if (frame.contentDocument === null)
    throw new Error("iframe document missing");
  return frame.contentDocument;
};

const cases = [
  {
    name: "document horizontal scroll",
    expected: "document-scroll",
    html: 'data-scroll-viewport=""',
    body: '<div style="width:800px;height:10px"></div>',
  },
  {
    name: "long text in a fixed height",
    expected: "unauthorized-overflow",
    body: '<div style="width:120px;height:20px">긴 글 긴 글 긴 글 긴 글 긴 글 긴 글 긴 글 긴 글</div>',
  },
  {
    name: "flex child without min-width zero",
    expected: "unauthorized-overflow",
    body: '<div style="display:flex;width:120px"><div style="white-space:nowrap">verylongunbreakableenglishwordverylongunbreakableenglishword</div></div>',
  },
  {
    name: "popover clipped by ancestor",
    expected: "clipped",
    body: '<div style="position:absolute;left:50px;top:50px;overflow:hidden;width:100px;height:80px"><div role="dialog" style="position:absolute;left:-20px;top:10px;width:40px;height:30px">팝업</div></div>',
  },
  {
    name: "truncation without full text tooltip",
    expected: "missing-tooltip",
    body: '<div data-truncate style="width:60px;overflow:hidden;white-space:nowrap;text-overflow:ellipsis">longlonglonglonglonglong</div>',
  },
  {
    name: "nested vertical scroll viewports",
    expected: "nested-scroll",
    body: '<div data-scroll-viewport style="width:180px;height:120px;overflow:auto"><div data-scroll-viewport style="width:150px;height:80px;overflow:auto"><div style="height:160px"></div></div><div style="height:100px"></div></div>',
  },
  {
    name: "portal outside the window",
    expected: "outside-window",
    body: '<div role="dialog" style="position:fixed;left:620px;top:40px;width:60px;height:40px">팝업</div>',
  },
  {
    name: "overlapping sibling controls",
    expected: "overlapping-controls",
    body: '<div><button style="width:100px;height:30px">하나</button><button style="width:100px;height:30px;margin-left:-20px">둘</button></div>',
  },
] as const;

afterEach(() => {
  document.querySelectorAll("iframe").forEach((frame) => frame.remove());
});

describe("overflow measurements", () => {
  it.each(cases)("isolates $name", async ({ body, expected, ...rest }) => {
    const doc = await makeDocument(body, "html" in rest ? rest.html : "");
    if (expected === "document-scroll")
      doc.body.setAttribute("data-scroll-viewport", "");
    const findings = inspectOverflow(doc);
    expect(findings.map((finding) => finding.kind)).toContain(expected);
    expect(findings.filter((finding) => finding.kind !== expected)).toEqual([]);
  });

  it("accepts contained controls and reachable viewport content", async () => {
    const doc = await makeDocument(
      '<div data-scroll-viewport style="overflow:auto;width:160px;height:80px"><div style="height:160px">내용</div></div><div><button>하나</button><button>둘</button></div>',
    );
    expect(inspectOverflow(doc)).toEqual([]);
  });

  it("accepts a tooltip containing the complete truncated text", async () => {
    const doc = await makeDocument(
      '<div data-truncate title="longlonglonglong" style="width:40px;overflow:hidden;white-space:nowrap">longlonglonglong</div>',
    );
    expect(inspectOverflow(doc)).toEqual([]);
  });

  it("detects vertical nesting through an intermediate horizontal viewport", async () => {
    const doc = await makeDocument(
      '<div data-scroll-viewport style="overflow-y:auto;width:180px;height:120px"><div data-scroll-viewport style="overflow-x:auto;overflow-y:hidden;width:170px;height:100px"><div data-scroll-viewport style="overflow-y:auto;width:150px;height:80px"><div style="height:160px"></div></div></div><div style="height:100px"></div></div>',
    );
    expect(inspectOverflow(doc).map((finding) => finding.kind)).toContain(
      "nested-scroll",
    );
  });

  it("permits the ChatInput textarea as the documented nested-scroll exception", async () => {
    const doc = await makeDocument(
      '<div data-scroll-viewport style="overflow:auto;width:180px;height:100px"><textarea class="chat-input" data-scroll-viewport style="overflow:auto;width:150px;height:80px">내용</textarea><div style="height:100px"></div></div>',
    );
    expect(inspectOverflow(doc).map((finding) => finding.kind)).not.toContain(
      "nested-scroll",
    );
  });

  it("still rejects another nested textarea viewport", async () => {
    const doc = await makeDocument(
      '<div data-scroll-viewport style="overflow:auto;width:180px;height:100px"><textarea data-scroll-viewport style="overflow:auto;width:150px;height:80px">내용</textarea><div style="height:100px"></div></div>',
    );
    expect(inspectOverflow(doc).map((finding) => finding.kind)).toContain(
      "nested-scroll",
    );
  });

  it("detects document vertical scroll independently", async () => {
    const doc = await makeDocument(
      '<div style="height:800px;width:10px"></div>',
      'data-scroll-viewport=""',
    );
    doc.body.setAttribute("data-scroll-viewport", "");
    expect([
      ...new Set(inspectOverflow(doc).map((finding) => finding.kind)),
    ]).toEqual(["document-scroll"]);
  });

  it("rejects a tooltip that omits part of the truncated text", async () => {
    const doc = await makeDocument(
      '<div data-truncate title="longlong" style="width:40px;overflow:hidden;white-space:nowrap">longlonglonglong</div>',
    );
    expect(inspectOverflow(doc).map((finding) => finding.kind)).toContain(
      "missing-tooltip",
    );
  });

  it.each([
    ["left", "left:-20px;top:10px"],
    ["right", "left:80px;top:10px"],
    ["top", "left:10px;top:-20px"],
    ["bottom", "left:10px;top:70px"],
  ])("detects clipping at the %s edge", async (_name, position) => {
    const doc = await makeDocument(
      `<div data-truncate title="팝업" style="position:absolute;left:100px;top:100px;overflow:hidden;width:100px;height:80px"><div style="position:absolute;${position};width:40px;height:30px">팝업</div></div>`,
    );
    expect(inspectOverflow(doc).map((finding) => finding.kind)).toContain(
      "clipped",
    );
  });

  it.each([
    ["left", "left:-1px;top:100px"],
    ["right", "left:620px;top:100px"],
    ["top", "left:100px;top:-1px"],
    ["bottom", "left:100px;top:460px"],
  ])("detects an overlay beyond the %s edge", async (_name, position) => {
    const doc = await makeDocument(
      `<div role="dialog" style="position:fixed;${position};width:40px;height:30px">팝업</div>`,
    );
    expect(inspectOverflow(doc).map((finding) => finding.kind)).toContain(
      "outside-window",
    );
  });

  it.each([
    ["left", "left:20px;top:40px"],
    ["right", "left:100px;top:40px"],
    ["top", "left:40px;top:20px"],
    ["bottom", "left:40px;top:60px"],
  ])("detects overlapping controls on the %s side", async (_name, position) => {
    const doc = await makeDocument(
      `<div style="position:relative;width:240px;height:160px"><button style="position:absolute;left:40px;top:40px;width:100px;height:40px">하나</button><button style="position:absolute;${position};width:60px;height:30px">둘</button></div>`,
    );
    expect(inspectOverflow(doc).map((finding) => finding.kind)).toContain(
      "overlapping-controls",
    );
  });

  it("allows adjacent controls that only touch at an edge", async () => {
    const doc = await makeDocument(
      '<div style="position:relative;width:240px;height:80px"><button style="position:absolute;left:0;top:0;width:100px;height:40px">하나</button><button style="position:absolute;left:100px;top:0;width:60px;height:40px">둘</button></div>',
    );
    expect(inspectOverflow(doc).map((finding) => finding.kind)).not.toContain(
      "overlapping-controls",
    );
  });
});

describe("protected stress data", () => {
  it("includes the required boundary-sized inputs", () => {
    expect(stressFixtures.company.length).toBe(40);
    expect(stressFixtures.company).not.toMatch(/\s/u);
    expect(stressFixtures.english.length).toBeGreaterThan(40);
    expect(stressFixtures.money).toBe("9876543210123");
    expect(stressFixtures.quantity).toBe("987654321.12345678");
    expect(stressFixtures.holdings).toHaveLength(200);
    expect(stressFixtures.paragraphs).toHaveLength(20);
    expect(stressFixtures.sources).toHaveLength(30);
    expect(stressFixtures.empty).toBe("");
  });
});
