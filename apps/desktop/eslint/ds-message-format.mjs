const emoji =
  /\p{Emoji_Presentation}|\p{Regional_Indicator}{2}|[#*\d]\uFE0F?\u20E3|\p{Extended_Pictographic}\uFE0F/u;
const catalogValue = (node) => {
  let child = node;
  for (
    let parent = node.parent;
    parent !== undefined && parent !== null;
    parent = parent.parent
  ) {
    if (parent.type === "Property" && parent.parent.type === "ObjectExpression")
      return parent.value === child;
    if (parent.type === "Program") return false;
    child = parent;
  }
  return false;
};
const literalText = (node) => {
  if (node.type === "Literal" && typeof node.value === "string")
    return node.value;
  if (node.type === "BinaryExpression" && node.operator === "+")
    return literalText(node.left) + literalText(node.right);
  if (node.type === "TemplateLiteral")
    return node.quasis
      .map(
        (q, i) =>
          (q.value.cooked ?? "") + literalText(node.expressions[i] ?? {}),
      )
      .join("");
  return "";
};
export const noMessageFormat = {
  meta: {
    type: "problem",
    schema: [],
    messages: {
      minus: "음수 문구의 빼기는 하이픈 대신 U+2212(−)를 쓴다.",
      emoji: "문구 목록 값에는 이모지 표현을 쓰지 않는다.",
    },
  },
  create(ctx) {
    const check = (node) => {
      if (!catalogValue(node)) return;
      const text = literalText(node);
      if (/-\d/u.test(text)) ctx.report({ node, messageId: "minus" });
      if (emoji.test(text)) ctx.report({ node, messageId: "emoji" });
    };
    return { Literal: check, TemplateLiteral: check, BinaryExpression: check };
  },
};
