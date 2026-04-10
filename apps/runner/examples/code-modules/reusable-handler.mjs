export async function branchByPriority(trigger, input, state, env, params) {
  const branch = params.route === 'priority' ? 'priority' : 'default';
  console.info('named export handler', { orderNo: params.orderNo, branch });

  return {
    output: {
      orderNo: params.orderNo,
      branch,
      source: 'named-export'
    },
    statePatch: {
      moduleResult: {
        orderNo: params.orderNo,
        branch,
        tenantId: env.tenantId
      }
    },
    branchKey: branch
  };
}

export async function auditOnly(trigger, input, state, env, params) {
  return {
    ok: true,
    requestId: params.requestId ?? null
  };
}
