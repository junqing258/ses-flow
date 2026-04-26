import { ElMessage } from "element-plus";

type ToastMessage = string | Error;

const normalizeMessage = (message: ToastMessage) =>
  message instanceof Error ? message.message : message;

export const toast = {
  success: (message: ToastMessage) =>
    ElMessage.success({ message: normalizeMessage(message) }),
  error: (message: ToastMessage) =>
    ElMessage.error({ message: normalizeMessage(message) }),
  info: (message: ToastMessage) =>
    ElMessage.info({ message: normalizeMessage(message) }),
  warning: (message: ToastMessage) =>
    ElMessage.warning({ message: normalizeMessage(message) }),
};
