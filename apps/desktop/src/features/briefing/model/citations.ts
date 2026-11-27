import type { BriefingSourceView } from "../../../shared/ipc";
import {
  parseFreeText,
  type EntityId,
  type ExternalUrl,
  type FreeText,
} from "../../../shared/lib";

type Citation = {
  readonly number: EntityId;
  readonly url: ExternalUrl;
  readonly title: FreeText;
};

export const briefingCitations = (
  sources: readonly BriefingSourceView[],
): readonly Citation[] =>
  sources.flatMap((source) => {
    if (source.cite === undefined || source.url === undefined) return [];
    const title =
      source.publisher === undefined
        ? undefined
        : parseFreeText(`${source.publisher} · ${source.title}`);
    return [
      {
        number: source.cite,
        url: source.url,
        title: title?.kind === "ok" ? title.value : source.title,
      },
    ];
  });
