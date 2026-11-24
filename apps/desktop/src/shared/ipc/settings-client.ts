import { err, type Result } from "../lib";
import type {
  AiSettings,
  AiTaskSetting,
  BudgetPercent,
  ScheduledJobSetting,
  ScheduledJobs,
} from "./ai-schema";
import type {
  AliasRow,
  NewsSettings,
  OptionalKey,
  SourceSetting,
  WakeCriteria,
} from "./news-schema";
import type {
  InvestmentProfile,
  OrderSwitch,
  OrderSwitchState,
} from "./profile-schema";
import type {
  AccountRef,
  ConfigPathText,
  ContactText,
  IpAddressText,
  IpcFailure,
  SecretText,
} from "./schema";
import type {
  AccountSelection,
  BackupResult,
  CopyResult,
  DeleteResult,
  ExportResult,
  Principles,
  PublicIp,
  TossConnection,
} from "./setup-schema";

type Reply<T> = Promise<Result<T, IpcFailure>>;

export type TossKeys = {
  readonly clientId: SecretText;
  readonly secret: SecretText;
};

type SetupClient = {
  readonly checkTossConnection: () => Reply<TossConnection>;
  readonly submitTossKeys: (keys: TossKeys) => Reply<TossConnection>;
  readonly publicIp: () => Reply<PublicIp>;
  readonly copyText: (value: IpAddressText) => Reply<CopyResult>;
};

type SystemClient = {
  readonly accountSelection: () => Reply<AccountSelection>;
  readonly selectAccount: (id: AccountRef) => Reply<AccountSelection>;
  readonly backupNow: () => Reply<BackupResult>;
  readonly exportData: () => Reply<ExportResult>;
  readonly deleteAllData: () => Reply<DeleteResult>;
};

type AiClient = {
  readonly aiSettings: () => Reply<AiSettings>;
  readonly setTaskModel: (setting: AiTaskSetting) => Reply<AiSettings>;
  readonly setBudgetPercent: (percent: BudgetPercent) => Reply<AiSettings>;
  readonly setClaudeConfigDir: (
    path: ConfigPathText | null,
  ) => Reply<AiSettings>;
  readonly scheduledJobs: () => Reply<ScheduledJobs>;
  readonly setScheduledJob: (
    setting: ScheduledJobSetting,
  ) => Reply<ScheduledJobs>;
};

type NewsClient = {
  readonly newsSettings: () => Reply<NewsSettings>;
  readonly setNewsSource: (setting: SourceSetting) => Reply<NewsSettings>;
  readonly setOptionalKey: (
    key: OptionalKey,
    secret: SecretText,
  ) => Reply<NewsSettings>;
  readonly setSecContact: (contact: ContactText | null) => Reply<NewsSettings>;
  readonly setWakeCriteria: (wake: WakeCriteria) => Reply<NewsSettings>;
  readonly setAliases: (aliases: readonly AliasRow[]) => Reply<NewsSettings>;
  readonly principles: () => Reply<Principles>;
  readonly setPrinciples: (principles: Principles) => Reply<Principles>;
};

type BasicClient = {
  readonly investmentProfile: () => Reply<InvestmentProfile>;
  readonly setInvestmentProfile: (
    profile: InvestmentProfile,
  ) => Reply<InvestmentProfile>;
  readonly orderSwitch: () => Reply<OrderSwitch>;
  readonly requestOrderSwitch: (state: OrderSwitchState) => Reply<OrderSwitch>;
};

export type SettingsClient = SetupClient &
  SystemClient &
  AiClient &
  NewsClient &
  BasicClient;

const noData: IpcFailure = { kind: "command", error: { kind: "no-data" } };

const unavailable = <T>(): Reply<T> => Promise.resolve(err(noData));

export const unavailableSettingsClient: SettingsClient = {
  checkTossConnection: unavailable,
  submitTossKeys: unavailable,
  publicIp: unavailable,
  copyText: unavailable,
  accountSelection: unavailable,
  selectAccount: unavailable,
  backupNow: unavailable,
  exportData: unavailable,
  deleteAllData: unavailable,
  aiSettings: unavailable,
  setTaskModel: unavailable,
  setBudgetPercent: unavailable,
  setClaudeConfigDir: unavailable,
  scheduledJobs: unavailable,
  setScheduledJob: unavailable,
  newsSettings: unavailable,
  setNewsSource: unavailable,
  setOptionalKey: unavailable,
  setSecContact: unavailable,
  setWakeCriteria: unavailable,
  setAliases: unavailable,
  principles: unavailable,
  setPrinciples: unavailable,
  investmentProfile: unavailable,
  setInvestmentProfile: unavailable,
  orderSwitch: unavailable,
  requestOrderSwitch: unavailable,
};
