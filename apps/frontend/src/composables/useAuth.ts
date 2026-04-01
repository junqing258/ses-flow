import { computed, ref } from "vue";

import {
  clearPersistedAccessToken,
  persistAccessToken,
  readPersistedAccessToken,
  resolveAuthStorage,
} from "@/lib/auth-storage";
import type {
  AuthDialogMode,
  AuthPayload,
  AuthSession,
  AuthUser,
  LoginPayload,
  RegisterPayload,
} from "@/types/auth";

const AUTH_BASE_URL = "/api/auth";

class ApiRequestError extends Error {
  status: number | null;

  constructor(message: string, status: number | null = null) {
    super(message);
    this.name = "ApiRequestError";
    this.status = status;
  }
}

const accessToken = ref<string | null>(readPersistedAccessToken(resolveAuthStorage()));
const user = ref<AuthUser | null>(null);
const session = ref<AuthSession | null>(null);
const dialogOpen = ref(false);
const dialogMode = ref<AuthDialogMode>("login");
const isHydrating = ref(false);
const isSubmitting = ref(false);
const isInitialized = ref(false);

let initializePromise: Promise<void> | null = null;

const resolveErrorMessage = (payload: unknown, fallback: string) => {
  if (!payload || typeof payload !== "object") {
    return fallback;
  }

  const candidate = (payload as { message?: unknown }).message;
  if (typeof candidate === "string" && candidate.trim()) {
    return candidate;
  }

  if (Array.isArray(candidate) && candidate.length > 0) {
    const firstText = candidate.find((item) => typeof item === "string" && item.trim());
    if (typeof firstText === "string") {
      return firstText;
    }
  }

  const nestedErrors = (payload as {
    errors?: { formErrors?: unknown; fieldErrors?: Record<string, unknown> };
  }).errors;

  if (Array.isArray(nestedErrors?.formErrors) && nestedErrors.formErrors.length > 0) {
    const firstFormError = nestedErrors.formErrors.find(
      (item) => typeof item === "string" && item.trim(),
    );
    if (typeof firstFormError === "string") {
      return firstFormError;
    }
  }

  if (nestedErrors?.fieldErrors && typeof nestedErrors.fieldErrors === "object") {
    for (const value of Object.values(nestedErrors.fieldErrors)) {
      if (Array.isArray(value) && typeof value[0] === "string" && value[0].trim()) {
        return value[0];
      }
    }
  }

  return fallback;
};

const applyAuthPayload = (payload: AuthPayload) => {
  accessToken.value = payload.accessToken;
  user.value = payload.user;
  session.value = payload.session;
  persistAccessToken(resolveAuthStorage(), payload.accessToken);
};

const clearAuthState = () => {
  accessToken.value = null;
  user.value = null;
  session.value = null;
  clearPersistedAccessToken(resolveAuthStorage());
};

const request = async <T>(path: string, init: RequestInit = {}) => {
  const headers = new Headers(init.headers ?? {});
  const isJsonBody = init.body !== undefined && !headers.has("Content-Type");

  if (isJsonBody) {
    headers.set("Content-Type", "application/json");
  }

  if (accessToken.value) {
    headers.set("Authorization", `Bearer ${accessToken.value}`);
  }

  let response: Response;
  try {
    response = await fetch(`${AUTH_BASE_URL}${path}`, {
      ...init,
      headers,
    });
  } catch {
    throw new ApiRequestError("Network request failed");
  }

  const contentType = response.headers.get("content-type") ?? "";
  const hasJsonBody = contentType.includes("application/json");
  const payload = hasJsonBody ? ((await response.json()) as unknown) : null;

  if (!response.ok) {
    throw new ApiRequestError(
      resolveErrorMessage(payload, "Authentication request failed"),
      response.status,
    );
  }

  return payload as T;
};

const refreshProfile = async () => {
  const payload = await request<{ user: AuthUser; session: AuthSession }>("/me", {
    method: "GET",
  });

  user.value = payload.user;
  session.value = payload.session;
  return payload;
};

const initialize = async () => {
  if (isInitialized.value) {
    return;
  }

  if (initializePromise) {
    return initializePromise;
  }

  initializePromise = (async () => {
    if (!accessToken.value) {
      isInitialized.value = true;
      initializePromise = null;
      return;
    }

    isHydrating.value = true;

    try {
      await refreshProfile();
    } catch (error) {
      if (error instanceof ApiRequestError && error.status === 401) {
        clearAuthState();
      }
    } finally {
      isHydrating.value = false;
      isInitialized.value = true;
      initializePromise = null;
    }
  })();

  return initializePromise;
};

const login = async (payload: LoginPayload) => {
  isSubmitting.value = true;

  try {
    const response = await request<AuthPayload>("/login", {
      method: "POST",
      body: JSON.stringify(payload),
    });

    applyAuthPayload(response);
    dialogOpen.value = false;
    isInitialized.value = true;
    return response;
  } finally {
    isSubmitting.value = false;
  }
};

const register = async (payload: RegisterPayload) => {
  isSubmitting.value = true;

  try {
    const response = await request<AuthPayload>("/register", {
      method: "POST",
      body: JSON.stringify({
        ...payload,
        displayName: payload.displayName?.trim() || undefined,
      }),
    });

    applyAuthPayload(response);
    dialogOpen.value = false;
    isInitialized.value = true;
    return response;
  } finally {
    isSubmitting.value = false;
  }
};

const logout = async () => {
  isSubmitting.value = true;

  try {
    if (accessToken.value) {
      await request<void>("/logout", {
        method: "POST",
      });
    }
  } finally {
    clearAuthState();
    dialogOpen.value = false;
    isInitialized.value = true;
    isSubmitting.value = false;
  }
};

const openAuthDialog = (mode: AuthDialogMode = "login") => {
  dialogMode.value = mode;
  dialogOpen.value = true;
};

const setDialogOpen = (open: boolean) => {
  dialogOpen.value = open;
};

const setDialogMode = (mode: AuthDialogMode) => {
  dialogMode.value = mode;
};

export const useAuth = () => ({
  accessToken,
  dialogMode,
  dialogOpen,
  initialize,
  isAuthenticated: computed(() => Boolean(accessToken.value && user.value && session.value)),
  isHydrating,
  isInitialized,
  isSubmitting,
  login,
  logout,
  openAuthDialog,
  register,
  session,
  setDialogMode,
  setDialogOpen,
  user,
});
