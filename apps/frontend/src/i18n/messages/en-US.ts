const enUS = {
  common: {
    language: "Language",
  },
  about: {
    eyebrow: "Project Setup",
    title: "App Internationalization",
    description: "The frontend now uses vue-i18n with browser locale detection, local persistence, and runtime language switching.",
    currentLocale: "Current locale",
    switcherLabel: "Switch language",
    cards: {
      detection: {
        title: "Auto detection",
        description: "On the first visit, the app selects the best matching locale from the browser preferences.",
      },
      persistence: {
        title: "Preference memory",
        description: "Manual changes are stored locally so the selected locale stays consistent after refresh.",
      },
      runtime: {
        title: "Runtime updates",
        description: "Changing the locale updates page copy and the html lang attribute, ready for more pages to adopt.",
      },
    },
  },
} as const;

export default enUS;
