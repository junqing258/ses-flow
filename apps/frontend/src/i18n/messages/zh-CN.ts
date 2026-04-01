const zhCN = {
  common: {
    language: "语言",
  },
  about: {
    eyebrow: "Project Setup",
    title: "应用国际化配置",
    description: "前端已接入 vue-i18n，支持浏览器语言探测、本地持久化，以及运行时切换语言。",
    currentLocale: "当前语言环境",
    switcherLabel: "切换语言",
    cards: {
      detection: {
        title: "自动探测",
        description: "首次进入应用时，会根据浏览器偏好语言自动匹配最合适的语言。",
      },
      persistence: {
        title: "偏好记忆",
        description: "手动切换后的语言会写入本地存储，刷新页面后依然保持一致。",
      },
      runtime: {
        title: "即时生效",
        description: "切换语言后会同步更新页面文案与 html lang 属性，方便后续扩展更多页面。",
      },
    },
  },
} as const;

export default zhCN;
