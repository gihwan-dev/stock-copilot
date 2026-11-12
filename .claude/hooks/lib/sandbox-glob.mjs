const escape = (text) => text.replace(/[.+^${}()|[\]\\]/gu, "\\$&");

export const globSource = (pattern) => {
  let source = "";
  for (let index = 0; index < pattern.length; index += 1) {
    const character = pattern[index];
    if (pattern.startsWith("**/", index)) {
      source += "(?:.*/)?";
      index += 2;
    } else if (pattern.startsWith("**", index)) {
      source += ".*";
      index += 1;
    } else if (character === "*") {
      source += "[^/]*";
    } else if (character === "?") {
      source += "[^/]";
    } else {
      source += escape(character);
    }
  }
  return source;
};

export const globExpression = (pattern) =>
  new RegExp(`^${globSource(pattern)}(?:/.*)?$`, "u");

export const hasGlob = (pattern) => /[*?[]/u.test(pattern);
