export default async function runCode(trigger, input, state, env, params) {
  const quantity = params.qty == null ? 0 : Number(params.qty);
  const branch = params.route === 'priority' ? 'priority' : 'default';

  console.log('code node start', { requestId: params.requestId, branch });

  return {
    output: {
      orderNo: params.orderNo,
      normalizedQty: quantity,
      route: params.route,
      branch,
      requestId: params.requestId
    },
    statePatch: {
      code: {
        orderNo: params.orderNo,
        normalizedQty: quantity,
        branch,
        requestId: params.requestId,
        tenantId: env.tenantId
      }
    },
    branchKey: branch
  };
}
