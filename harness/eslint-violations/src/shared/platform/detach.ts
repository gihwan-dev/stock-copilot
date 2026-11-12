const ignore = (): undefined => undefined;
export const detach = <T>(task: Promise<T>): void => {
  task.then(ignore, ignore);
};
