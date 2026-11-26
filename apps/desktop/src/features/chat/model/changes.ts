import {
  chatChangeTopics,
  type ChatChangeTopic,
  type ChatToolStatus,
  type ChatToolTopic,
} from "../../../shared/ipc";

export const withChange = (
  changes: readonly ChatChangeTopic[],
  topic: ChatToolTopic,
  status: ChatToolStatus,
): readonly ChatChangeTopic[] => {
  const change = chatChangeTopics.find((known) => known === topic);
  return change === undefined ||
    status !== "succeeded" ||
    changes.includes(change)
    ? changes
    : [...changes, change];
};
