import { isDeepStrictEqual } from "node:util";
import { buildResponses } from "./portfolio-model.mjs";

const rising = /상승|상방|강세|오르|올랐|오름/;
const falling = /하락|하방|약세|내리|내렸|떨어|약해/;
const conditional = /면|경우|때는|때에는|때만/;
const certain = /반드시|무조건|틀림없이|확실히|분명히|보장|확정적|(?:백|100)\s*퍼센트|(?:한|두|세|네|다섯|열|몇|수|\d+)\s*배(?:로|가|씩|까지|\s|[.,])/;
const forecastWithoutEvidence = /상승(?:합|할|합니다|세|이 우세)|하락(?:합|할|합니다|세|이 우세)|횡보(?:합|할|세)|[오내]릅니다|오를 겁|내릴 겁|오를 것|내릴 것|강세|약세|상방 우위|하방 우위|[\d.]+\s*(?:%|퍼센트)/;

function dated(sentence, iso) {
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(iso ?? "");
  if (!match) return false;
  const month = Number(match[2]);
  const day = Number(match[3]);
  return new RegExp(`(?:^|[^0-9])(?:0?${month}(?:월\\s*|[./-]))0?${day}(?:일|[^0-9]|$)`).test(sentence);
}

function readMatches(call, step, catalog, failures) {
  if (!call || call.name !== step.name || call.isError !== step.isError) return false;
  if (!Object.entries(step.args).every(([key, pattern]) =>
    new RegExp(pattern).test(String(call.args?.[key] ?? "")))) return false;
  if (call.name === "request_web_research" && Object.keys(call.args).length !== Object.keys(step.args).length) return false;
  if (call.isError) {
    const failure = failures?.[call.name];
    return failure != null && call.result?.code === failure.code &&
      call.result?.message === failure.message && call.result?.retryable === failure.retryable;
  }
  const payload = catalog[call.name];
  const expected = payload?.__bySymbol ? payload.__bySymbol[call.args.symbol] : payload;
  return expected != null && isDeepStrictEqual(call.result, expected);
}

export function outlookChecks(scenario, answer, calls) {
  const steps = scenario.expect.research_steps;
  const reads = calls.filter((call) => steps.some((step) => step.name === call.name));
  const catalog = buildResponses(scenario);
  const evidence = reads.length === steps.length && steps.every((step, index) =>
    readMatches(reads[index], step, catalog, scenario.failures));
  const checks = { research_evidence: evidence, conditional_language: !certain.test(answer) };
  if (!evidence) return checks;
  const sentences = answer.split(/(?<=[.!?。])\s+/);
  const candles = reads.find((read) => read.name === "get_candles" && !read.isError)?.result;
  const research = reads.find((read) => read.name === "request_web_research" && !read.isError)?.result;
  if (!candles && !research) {
    return { ...checks, no_unsupported_forecast: !forecastWithoutEvidence.test(answer) };
  }
  const latest = candles?.items?.at(0);
  const oldest = candles?.items?.at(-1);
  const trend = candles && Number(latest.close.amount) > Number(oldest.close.amount) ? rising : falling;
  checks.direction = trend.test(sentences[0]);
  checks.reversal_condition = sentences.some((sentence) => conditional.test(sentence));
  if (candles) {
    const quotedCloses = [...answer.matchAll(/(?:최근|최신|마지막|25일)\s*종가(?:는|가|를|로|\s)*\$?([\d,]+(?:\.\d+)?)[ ]*(원|달러|USD|KRW)/g)];
    checks.quoted_close = quotedCloses.every(([, amount, unit]) =>
      Number(amount.replaceAll(",", "")) === Number(latest.close.amount) &&
      (["원", "KRW"].includes(unit) ? "KRW" : "USD") === latest.close.currency);
    const source = candles.symbol === "SPY" ? /SPY/i : /KODEX\s*200|코덱스\s*200|069500/i;
    checks.candle_basis = sentences.some((sentence) => dated(sentence, latest.trading_date) &&
      source.test(sentence) && trend.test(sentence));
    const news = reads.find((read) => read.name === "search_news")?.result?.items?.[0];
    if (news) checks.opposing_evidence = sentences.some((sentence) =>
      sentence.includes(news.source) && dated(sentence, news.published_at) && /수출.*(?:증가|늘|개선)/.test(sentence));
  } else {
    const reportDate = research.summary?.untrusted_text?.match(/\d{4}-\d{2}-\d{2}/)?.[0];
    checks.research_basis = sentences.some((sentence) =>
      /경제통계/.test(sentence) && /수출.*(?:감소|줄|둔화)/.test(sentence) && dated(sentence, reportDate));
  }
  return checks;
}
