const en = {
  cancelling: "Cancelling download…",
  checking: "Checking component…", unsupported: "Available on Windows x64 only.",
  missing: "The Windows control component needs to be downloaded. Chat remains available.",
  download: "Download and enable", apply: "Apply change", cancel: "Cancel", retry: "Retry",
  size: (mib: string) => `First use downloads ${mib} MB. It is kept for offline use.`,
  downloading: "Downloading Windows control component", verifying: "Verifying component…",
  ready: "Component installed", waiting: "Waiting for tasks to finish and the workbench to close before applying this change.",
  applying: "Applying component settings…", failed: "Component operation failed. You can retry; chat is still available.",
  checksum: "Component verification failed. The downloaded file was not installed. Please retry.",
  storage: "Cannot install the component. Check available disk space and folder access, then retry.",
  cancelDownload: "Cancel download", enable: "Enable Windows UI control?", disable: "Disable Windows UI control?",
  safeApply: "The change takes effect after current tasks finish and the workbench is closed.",
};
type Text = typeof en;
const zh: Text = {
  cancelling: "正在取消下载…",
  checking: "正在检查组件…", unsupported: "仅支持 Windows x64。",
  missing: "需要下载 Windows 操作组件，普通聊天不受影响。",
  download: "下载并启用", apply: "应用更改", cancel: "取消", retry: "重试",
  size: (mib) => `首次使用需下载 ${mib} MB，下载后可离线复用。`,
  downloading: "正在下载 Windows 操作组件", verifying: "正在校验组件…",
  ready: "组件已安装", waiting: "等待任务结束并关闭工作台后生效。",
  applying: "正在应用组件设置…", failed: "组件操作失败，可以重试；普通聊天不受影响。",
  checksum: "组件校验失败，下载文件未安装，请重试。",
  storage: "无法安装组件，请检查磁盘空间和目录访问权限后重试。",
  cancelDownload: "取消下载", enable: "启用 Windows 界面操作？", disable: "停用 Windows 界面操作？",
  safeApply: "当前任务结束并关闭工作台后，更改才会生效。",
};
const ja: Text = {
  cancelling: "ダウンロードを中止しています…",
  checking: "コンポーネントを確認中…", unsupported: "Windows x64 のみ対応しています。",
  missing: "Windows 操作コンポーネントのダウンロードが必要です。チャットは引き続き利用できます。",
  download: "ダウンロードして有効化", apply: "変更を適用", cancel: "キャンセル", retry: "再試行",
  size: (mib) => `初回は ${mib} MB をダウンロードします。以降はオフラインでも利用できます。`,
  downloading: "Windows 操作コンポーネントをダウンロード中", verifying: "コンポーネントを検証中…",
  ready: "インストール済み", waiting: "タスクの完了とワークベンチが閉じられるのを待っています。",
  applying: "設定を適用中…", failed: "操作に失敗しました。再試行できます。チャットには影響しません。",
  checksum: "検証に失敗したため、インストールしませんでした。再試行してください。",
  storage: "空き容量とフォルダーへのアクセス権を確認して再試行してください。",
  cancelDownload: "ダウンロードを中止", enable: "Windows UI 操作を有効にしますか？", disable: "Windows UI 操作を無効にしますか？",
  safeApply: "実行中のタスクが完了し、ワークベンチを閉じると適用されます。",
};
const ko: Text = {
  cancelling: "다운로드 취소 중…",
  checking: "구성 요소 확인 중…", unsupported: "Windows x64에서만 사용할 수 있습니다.",
  missing: "Windows 제어 구성 요소를 다운로드해야 합니다. 채팅은 계속 사용할 수 있습니다.",
  download: "다운로드 후 활성화", apply: "변경 적용", cancel: "취소", retry: "다시 시도",
  size: (mib) => `처음 사용할 때 ${mib} MB를 다운로드합니다. 이후에는 오프라인에서도 사용할 수 있습니다.`,
  downloading: "Windows 제어 구성 요소 다운로드 중", verifying: "구성 요소 검증 중…",
  ready: "구성 요소 설치됨", waiting: "작업이 끝나고 워크벤치가 닫힐 때까지 기다립니다.",
  applying: "설정 적용 중…", failed: "작업에 실패했습니다. 다시 시도할 수 있으며 채팅에는 영향이 없습니다.",
  checksum: "검증에 실패하여 설치하지 않았습니다. 다시 시도하세요.",
  storage: "디스크 공간과 폴더 접근 권한을 확인한 후 다시 시도하세요.",
  cancelDownload: "다운로드 취소", enable: "Windows UI 제어를 활성화할까요?", disable: "Windows UI 제어를 비활성화할까요?",
  safeApply: "현재 작업이 끝나고 워크벤치가 닫히면 적용됩니다.",
};
export function windowsComponentText(language: string): Text {
  return language === "zh-CN" ? zh : language === "ja-JP" ? ja : language === "ko-KR" ? ko : en;
}
