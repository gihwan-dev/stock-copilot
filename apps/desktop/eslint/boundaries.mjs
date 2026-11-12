export const ELEMENTS = [
  { type: "app", pattern: "src/app" },
  {
    type: "feature-ui",
    pattern: "src/features/*/ui",
    capture: ["featureName"],
  },
  {
    type: "feature-model",
    pattern: "src/features/*/model",
    capture: ["featureName"],
  },
  { type: "feature", pattern: "src/features/*", capture: ["featureName"] },
  { type: "ds", pattern: "src/shared/ds" },
  { type: "ipc", pattern: "src/shared/ipc" },
  { type: "platform", pattern: "src/shared/platform" },
  { type: "lib", pattern: "src/shared/lib" },
].map((element) => ({ ...element, partialMatch: false }));

export const ENTRY_FILE = "index.ts";

export const FEATURE_ENTRIES = ["ui", "model", "index.ts", "messages.ts"];

export const ELEMENT_DEPENDENCIES = {
  app: ["feature", "ds", "ipc", "platform", "lib"],
  feature: ["ds", "ipc", "platform", "lib"],
  "feature-ui": ["ds", "ipc", "platform", "lib"],
  "feature-model": ["ds", "ipc", "platform", "lib"],
  ds: ["lib"],
  ipc: ["lib"],
  platform: ["ipc", "lib"],
  lib: [],
};

export const FEATURE_DEPENDENCIES = {
  feature: { "feature-ui": "**", "feature-model": "**" },
  "feature-ui": { "feature-model": "**", feature: "messages.ts" },
  "feature-model": {},
};

export const PACKAGE_LOCATIONS = {
  "react-dom": ["src/app/main.tsx"],
  "@tauri-apps/api": ["src/shared/ipc/**"],
  zod: ["src/shared/ipc/**", "src/shared/lib/brand.ts"],
};

export const BOUNDARY_TABLES = {
  ELEMENTS,
  ENTRY_FILE,
  FEATURE_ENTRIES,
  ELEMENT_DEPENDENCIES,
  FEATURE_DEPENDENCIES,
  PACKAGE_LOCATIONS,
};

const sameFeature = { featureName: "{{ from.element.captured.featureName }}" };

const featurePolicies = (dependencies) =>
  Object.entries(dependencies).flatMap(([from, targets]) =>
    Object.entries(targets).map(([to, fileInternalPath]) => ({
      from: { element: { type: from } },
      allow: {
        to: { element: { type: to, captured: sameFeature, fileInternalPath } },
      },
    })),
  );

const elementPolicies = (dependencies, entryFile) =>
  Object.entries(dependencies)
    .filter(([, targets]) => targets.length > 0)
    .map(([type, targets]) => ({
      from: { element: { type } },
      allow: {
        to: { element: { types: targets, fileInternalPath: entryFile } },
      },
    }));

export const BOUNDARY_MESSAGES = {
  element:
    "{{ from.element.types.[0] }}에서 {{ to.element.types.[0] }}의 {{ to.element.fileInternalPath }} 가져오기 금지: architecture 5장 폴더 의존 표의 방향으로, 다른 요소는 index.ts로만 가져온다.",
  package:
    "{{ to.module.source }} 가져오기 금지: 이 패키지는 정해진 위치의 파일에서만 가져온다.",
  core: "Node 내장 모듈 {{ to.module.source }} 가져오기 금지: UI는 IPC로만 바깥과 통신한다.",
  unknown:
    "src의 요소(app, features/*, shared/*) 밖 파일 {{ dependency.source }} 가져오기 금지.",
};

const packagePolicies = (locations) =>
  Object.entries(locations).flatMap(([source, paths]) => [
    {
      disallow: { to: { module: { origin: "external", source } } },
      message: BOUNDARY_MESSAGES.package,
    },
    {
      from: { file: { path: paths } },
      allow: { to: { module: { origin: "external", source } } },
    },
  ]);

export const boundaryPolicies = ({
  ENTRY_FILE: entryFile,
  ELEMENT_DEPENDENCIES: dependencies,
  FEATURE_DEPENDENCIES: features,
  PACKAGE_LOCATIONS: locations,
}) => [
  ...elementPolicies(dependencies, entryFile),
  ...featurePolicies(features),
  { allow: { to: { module: { origin: "external" } } } },
  ...packagePolicies(locations),
  {
    disallow: { to: { module: { origin: "core" } } },
    message: BOUNDARY_MESSAGES.core,
  },
  {
    disallow: {
      to: { module: { origin: "local" }, element: { isUnknown: true } },
    },
    message: BOUNDARY_MESSAGES.unknown,
  },
];

export const boundarySettings = ({ ELEMENTS: elements }, rootPath) => ({
  "boundaries/root-path": rootPath,
  "boundaries/legacy-templates": false,
  "boundaries/elements": elements,
  "import/resolver": { node: { extensions: [".ts", ".tsx"] } },
});

export const boundaryRules = (tables) => ({
  "boundaries/no-unknown-files": "error",
  "boundaries/no-unknown-dependencies": "error",
  "boundaries/dependencies": [
    "error",
    {
      default: "disallow",
      message: BOUNDARY_MESSAGES.element,
      checkAllOrigins: true,
      checkUnknownLocals: true,
      policies: boundaryPolicies(tables),
    },
  ],
});
