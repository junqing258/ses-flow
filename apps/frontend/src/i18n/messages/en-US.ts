const enUS = {
  common: {
    language: "Language",
  },
  auth: {
    actions: {
      login: "Log in",
      register: "Register",
      logout: "Log out",
      cancel: "Cancel",
      close: "Close",
      manageAccount: "Manage account",
      processing: "Working...",
    },
    status: {
      authenticated: "Authenticated",
      guest: "Guest",
    },
    fields: {
      displayName: "Display name",
      email: "Email",
      password: "Password",
    },
    placeholders: {
      displayName: "Enter a display name",
      email: "name@example.com",
      password: "At least 8 characters",
    },
    dialog: {
      eyebrow: "Access Control",
      loginTitle: "Sign in to the workflow hub",
      loginDescription: "Use your account to enter the workspace and load role-aware pages and tenant context.",
      registerTitle: "Create a new access account",
      registerDescription: "Complete a basic registration now and extend it later with tenant, warehouse, and role assignment.",
      accountTitle: "Current authenticated session",
      accountDescription: "You are signed in. Review the active session or sign out from this panel.",
      featureOneTitle: "Unified entry",
      featureOneDescription: "Ready for tenant switching, role dashboards, and finer-grained page authorization.",
      featureTwoTitle: "Secure session",
      featureTwoDescription: "The frontend persists the Bearer token and restores the session when the app starts.",
    },
    account: {
      displayName: "Display name",
      email: "Email",
      role: "Role",
      expiresAt: "Session expires",
      lastLogin: "Last login",
      unavailable: "Unavailable",
    },
    feedback: {
      loginSuccess: "Signed in successfully",
      registerSuccess: "Account created and signed in",
      logoutSuccess: "Signed out successfully",
      genericError: "Authentication request failed. Please try again.",
    },
    fallbackName: "No display name",
    roles: {
      SUPER_ADMIN: "Super admin",
      ADMIN: "Admin",
      MANAGER: "Manager",
      OPERATOR: "Operator",
      VIEWER: "Viewer",
    },
  },
  about: {
    eyebrow: "Project Setup",
    title: "App Internationalization",
    description: "The frontend now uses vue-i18n with browser locale detection, local persistence, and runtime language switching.",
    authPanel: {
      title: "Authentication",
      loggedInHeading: "Account ready",
      loggedOutHeading: "Auth dialog connected",
      loggedInDescription: "This page now talks to the backend auth module and can surface session details for future tenant and role flows.",
      loggedOutDescription: "You can now open an auth dialog in the frontend to register, log in, and restore the session after refresh.",
      benefitsTitle: "What this version supports",
      benefits: {
        one: "Register, log in, log out, and restore the session after refresh",
        two: "A shared dialog entry point that other pages can reuse later",
        three: "Visible placeholders for backend role and session metadata",
      },
    },
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
