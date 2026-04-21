const TROUBLESHOOT_TEMPLATES: Record<string, Record<string, string[]>> = {
  "wms-sorting": {
    HTTP_ERROR: [
      "检查 WMS 接口 /pick-task 是否可用",
      "确认 HTTP 状态码、返回体和鉴权头是否符合预期",
      "检查现场网络策略和目标服务白名单配置",
    ],
    RESUME_MISMATCH: [
      "确认回调 URL 是否指向当前环境",
      "检查外部系统回调里的 requestId 或 taskId 是否与创建时一致",
      "排查是否存在重复回调或串单回调",
    ],
  },
};

export const getTroubleshootTemplateSteps = (
  workflowKey: string,
  errorCode?: string | null,
) => {
  if (!errorCode) {
    return [];
  }

  return TROUBLESHOOT_TEMPLATES[workflowKey]?.[errorCode] ?? [];
};
