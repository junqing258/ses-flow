<template>
  <section
    class="relative h-screen w-full overflow-hidden bg-[#f4f4f5] text-slate-800"
  >
    <!-- Main Canvas takes full absolute space -->
    <main
      class="workflow-canvas absolute inset-0 z-0 h-full w-full"
      @dragenter.prevent="handleCanvasDragEnter"
      @dragover.prevent="handleCanvasDragOver"
      @drop.prevent="handleCanvasDrop"
    >
      <VueFlow
        :nodes="nodes"
        :edges="edges"
        class="h-full w-full"
        @connect="handleConnect"
        @node-click="handleNodeClick"
        @pane-click="handlePaneClick"
      >
        <template #node-workflow-card="nodeProps">
          <WorkflowCanvasNode v-bind="nodeProps" />
        </template>
        <template #node-terminal="nodeProps">
          <WorkflowTerminalNode v-bind="nodeProps" />
        </template>
        <template #node-branch-chip="nodeProps">
          <WorkflowBranchChipNode v-bind="nodeProps" />
        </template>
      </VueFlow>

      <div class="pointer-events-none absolute inset-0 z-10">
        <div
          v-if="isCanvasDropTarget"
          class="absolute inset-4 rounded-[28px] border-2 border-dashed border-[#4f6af5]/55 bg-[#4f6af5]/5 shadow-[inset_0_0_0_1px_rgba(79,106,245,0.05)]"
        >
          <div class="absolute inset-x-0 top-6 flex justify-center">
            <div
              class="rounded-full bg-white/92 px-4 py-2 text-xs font-semibold tracking-[0.03em] text-[#4f6af5] shadow-sm"
            >
              松开鼠标，将节点放入画布
            </div>
          </div>
        </div>
      </div>
    </main>

    <!-- Floating Top Header -->
    <header
      class="pointer-events-none absolute inset-x-0 top-4 z-20 flex h-14 items-center justify-between px-6"
    >
      <div class="flex items-center gap-3 pointer-events-auto">
        <Button
          variant="ghost"
          size="icon"
          class="h-8 w-8 text-slate-500 rounded-full hover:bg-slate-200"
          @click="handleBackToList"
        >
          <ChevronLeft class="h-5 w-5" />
        </Button>
        <span class="text-[16px] font-semibold tracking-tight text-slate-900">{{
          workflowTitle
        }}</span>
        <span
          class="rounded-full bg-slate-200/80 px-2 py-0.5 text-[11px] font-semibold text-slate-600"
          >{{ workflowStatusLabel }}</span
        >
        <Button
          v-if="persistedWorkflowId"
          variant="ghost"
          class="h-8 gap-1.5 rounded-full px-3 text-sm font-medium text-slate-600 hover:bg-slate-200"
          @click="handleOpenWorkflowRuns"
        >
          <LoaderCircle class="h-3.5 w-3.5" />
          查看运行
        </Button>
      </div>

      <div
        class="absolute left-1/2 -translate-x-1/2 flex h-9 items-center rounded-full bg-white p-1 shadow-sm ring-1 ring-slate-100 pointer-events-auto"
      >
        <button
          class="flex h-7 w-11 items-center justify-center rounded-full transition-colors"
          :class="
            isEditMode
              ? 'bg-slate-100 text-slate-800'
              : 'text-slate-400 hover:bg-slate-50 hover:text-slate-800'
          "
          @click="handlePageModeChange('edit')"
        >
          <Pencil class="h-3.5 w-3.5" />
        </button>
        <button
          class="flex h-7 w-11 items-center justify-center rounded-full transition-colors"
          :class="
            isRunMode
              ? 'bg-slate-100 text-slate-800'
              : 'text-slate-400 hover:bg-slate-50 hover:text-slate-800'
          "
          @click="handlePageModeChange('run')"
        >
          <Play class="h-3.5 w-3.5" />
        </button>
        <button
          class="flex h-7 w-11 items-center justify-center rounded-full transition-colors"
          :class="
            isAiMode
              ? 'bg-slate-100 text-slate-800'
              : 'text-slate-400 hover:bg-slate-50 hover:text-slate-800'
          "
          @click="handlePageModeChange('ai')"
        >
          <Bot class="h-3.5 w-3.5" />
        </button>
      </div>

      <div class="flex items-center gap-1.5 pointer-events-auto">
        <Button
          variant="ghost"
          size="icon"
          class="h-8 w-8 text-slate-500 rounded-full hover:bg-slate-200"
        >
          <MoreHorizontal class="h-4 w-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          class="h-8 w-8 text-slate-500 rounded-full hover:bg-slate-200"
        >
          <Settings class="h-4 w-4" />
        </Button>
        <Button
          variant="ghost"
          class="h-8 gap-1.5 px-3 text-sm font-medium text-slate-600 rounded-full hover:bg-slate-200"
        >
          <Compass class="h-4 w-4" />
          Evaluate
        </Button>
        <Button
          variant="ghost"
          class="h-8 gap-1.5 px-3 text-sm font-medium text-slate-600 rounded-full hover:bg-slate-200"
          @click="handleExportJson"
        >
          <Code class="h-4 w-4" />
          JSON
        </Button>
        <Button
          class="ml-1 h-8 rounded-full bg-slate-900 px-4 text-[13px] font-medium text-white hover:bg-slate-800 shadow-sm disabled:cursor-not-allowed disabled:opacity-60"
          :disabled="isPublishing"
          @click="handlePublish"
        >
          {{ publishButtonLabel }}
        </Button>
      </div>
    </header>

    <WorkflowRunListDialog
      :open="isWorkflowRunListOpen"
      :workflow-id="persistedWorkflowId ?? ''"
      :workflow-name="workflowTitle"
      @update:open="handleWorkflowRunListOpenChange"
      @select-run="handleOpenWorkflowRunFromList"
    />

    <aside
      v-if="isAiMode"
      class="pointer-events-auto absolute right-6 top-24 bottom-auto z-10 flex max-h-[calc(100vh-7.5rem)] w-[320px] flex-col overflow-hidden rounded-[20px] bg-white/95 backdrop-blur shadow-sm ring-1 ring-slate-100/50"
    >
      <div class="border-b border-slate-100 px-4 py-4">
        <div class="flex items-start justify-between gap-3">
          <div>
            <p class="text-[14px] font-semibold text-slate-900">
              AI 工作流编辑助手
            </p>
            <p class="mt-1 text-[11px] leading-5 text-slate-500">
              Web 端只负责展示 `runner_base_url`、`session_id` 和预览结果。
              会话编辑动作在 Claude Code 中完成，并由它通过 `ses-flow-skill`
              推送预览。
            </p>
          </div>
          <span
            class="rounded-full px-2.5 py-1 text-[11px] font-semibold text-nowrap"
            :class="
              assistantConnectionState === 'connected'
                ? 'bg-emerald-50 text-emerald-700'
                : assistantConnectionState === 'connecting'
                  ? 'bg-amber-50 text-amber-700'
                  : 'bg-slate-100 text-slate-600'
            "
            >{{ assistantConnectionLabel }}</span
          >
        </div>
      </div>

      <div class="min-h-0 flex-1 space-y-4 overflow-y-auto px-4 py-4">
        <div
          class="rounded-[18px] border border-slate-200/80 bg-slate-50/70 p-3"
        >
          <p class="text-xs font-semibold tracking-wide text-slate-500">
            会话状态
          </p>
          <p class="mt-2 text-sm font-medium text-slate-900">
            {{
              isCreatingAssistantSession
                ? "正在创建 AI 会话"
                : hasAssistantSession
                  ? "会话已就绪"
                  : "等待创建会话"
            }}
          </p>
          <p class="mt-1 text-[11px] leading-5 text-slate-500">
            最近同步：{{ assistantSessionUpdatedLabel }}
          </p>
        </div>

        <div
          class="rounded-[14px] border border-slate-200/80 bg-white px-3 py-2"
        >
          <p
            class="shrink-0 pt-0.5 text-xs font-semibold tracking-wide text-slate-500"
          >
            runner_base_url: {{ assistantRunnerBaseUrl }}
          </p>
          <p
            class="shrink-0 pt-0.5 text-xs font-semibold tracking-wide text-slate-500"
          >
            session_id: {{ assistantSessionId || "(创建中)" }}
          </p>
        </div>
        <div class="space-y-1.5">
          <p class="text-[11px] leading-5 text-slate-500">
            首次请把 `runner_base_url` 和 `session_id` 一起提供给 Claude
            Code，后续它会用这个前缀拼接会话接口地址。
          </p>
        </div>

        <!--  <div class="rounded-[18px] border border-slate-200/80 bg-white p-3">
          <p class="text-xs font-semibold tracking-wide text-slate-500">
            Claude Code 调用提示
          </p>
          <pre
            class="mt-3 overflow-x-auto rounded-2xl bg-slate-950 px-3 py-3 text-[11px] leading-5 text-slate-100"
          ><code>runner_base_url: {{ assistantRunnerBaseUrl }}
session_id: {{ assistantSessionId || "(创建中)" }}
skill: ses-flow-skill
update: PUT {{ assistantRunnerBaseUrl }}/edit-sessions/{{ assistantSessionId || ":session_id" }}
preview: WS {{ assistantRunnerBaseUrl }}/edit-sessions/{{ assistantSessionId || ":session_id" }}/ws</code></pre>
          <p class="mt-3 text-[11px] leading-5 text-slate-500">
            AI 模式下 Web 侧不提供输入框、创建按钮或同步按钮；首次请把
            `runner_base_url` 和 `session_id` 一起提供给 Claude Code，它会据此拼接接口并更新临时会话，runner
            校验通过后会自动刷新这里的画布预览。
          </p>
        </div>

        <div
          class="rounded-[18px] border border-slate-200/80 bg-slate-50/70 px-3 py-3 text-[12px] leading-5 text-slate-600"
        >
          AI 模式已锁定 Web 编辑。请在 Claude Code
          中继续增删节点、改映射和调整分支。
        </div> -->

        <div
          v-if="assistantSessionError"
          class="rounded-[18px] border border-rose-100 bg-rose-50 px-3 py-3 text-[12px] leading-5 text-rose-700"
        >
          {{ assistantSessionError }}
        </div>
      </div>
    </aside>

    <!-- Floating Left Panel -->
    <aside
      v-if="isEditMode"
      class="pointer-events-auto absolute left-6 top-24 bottom-auto z-10 flex max-h-[calc(100vh-7.5rem)] w-[240px] flex-col overflow-hidden rounded-[20px] bg-white/95 backdrop-blur shadow-sm ring-1 ring-slate-100/50"
    >
      <div class="min-h-0 flex-1 overflow-y-auto py-3 px-2">
        <div
          v-for="category in filteredCategories"
          :key="category.id"
          class="mb-4 last:mb-0"
        >
          <div class="mb-1.5 px-3 text-[11px] font-medium text-slate-400">
            {{ category.label }}
          </div>
          <div class="space-y-0.5">
            <button
              v-for="item in category.items"
              :key="item.id"
              type="button"
              draggable="true"
              class="flex w-full cursor-grab items-center gap-2.5 rounded-xl px-2.5 py-1.5 text-left transition-colors active:cursor-grabbing"
              :class="
                selectedNodeId && selectedNodeData?.kind === item.kind
                  ? 'bg-slate-50'
                  : 'hover:bg-slate-50'
              "
              @click="focusPaletteItem(item.kind)"
              @dragstart="handlePaletteDragStart($event, item.id)"
              @dragend="handlePaletteDragEnd"
            >
              <div
                class="flex h-6.5 w-6.5 items-center justify-center rounded-lg bg-slate-100"
                :style="{
                  color: item.accent,
                  backgroundColor: `${item.accent}15`,
                }"
              >
                <component :is="resolveIcon(item.icon)" class="h-3.5 w-3.5" />
              </div>
              <div class="min-w-0 flex-1">
                <p class="truncate text-[13px] font-medium text-slate-700">
                  {{ item.label }}
                </p>
              </div>
            </button>
          </div>
        </div>
      </div>
    </aside>

    <aside
      v-else
      class="pointer-events-auto absolute left-6 top-24 bottom-auto z-10 flex max-h-[calc(100vh-7.5rem)] w-[320px] flex-col overflow-hidden rounded-[20px] bg-white/95 backdrop-blur shadow-sm ring-1 ring-slate-100/50"
    >
      <div
        class="flex h-[84px] shrink-0 items-center justify-between border-b border-slate-50 px-4"
      >
        <div>
          <p class="text-[14px] font-semibold text-slate-900">运行配置</p>
          <p class="mt-1 text-[11px] leading-5 text-slate-400">
            同步当前画布到 Runner 后立即执行，适合联调节点映射和分支结果。
          </p>
        </div>
        <span
          class="rounded-full bg-slate-100 px-2.5 py-1 text-[11px] font-semibold text-slate-600 truncate"
          >{{ runnerTriggerSummaryLabel }}</span
        >
      </div>

      <div class="min-h-0 flex-1 space-y-4 overflow-y-auto px-4 py-4">
        <div
          class="rounded-[18px] border border-slate-200/80 bg-slate-50/70 p-3"
        >
          <p class="text-xs font-semibold tracking-wide text-slate-500">
            触发载荷
          </p>
          <div
            class="mt-3 flex rounded-full bg-white p-1 ring-1 ring-slate-200"
          >
            <button
              type="button"
              class="flex-1 rounded-full px-3 py-1.5 text-xs font-medium transition-colors"
              :class="
                runDraft.triggerMode === 'manual'
                  ? 'bg-slate-900 text-white'
                  : 'text-slate-500 hover:text-slate-800'
              "
              @click="handleRunDraftUpdate('triggerMode', 'manual')"
            >
              Manual
            </button>
            <button
              type="button"
              class="flex-1 rounded-full px-3 py-1.5 text-xs font-medium transition-colors"
              :class="
                runDraft.triggerMode === 'webhook'
                  ? 'bg-slate-900 text-white'
                  : 'text-slate-500 hover:text-slate-800'
              "
              @click="handleRunDraftUpdate('triggerMode', 'webhook')"
            >
              Webhook
            </button>
          </div>
          <p class="mt-3 text-[11px] leading-5 text-slate-500">
            `Manual` 只发送 `body`，`Webhook` 会附带 `headers +
            body`，方便模拟真实入口。
          </p>
        </div>

        <div class="space-y-1.5">
          <label
            class="block text-xs font-semibold tracking-wide text-slate-500"
            >Trigger Body</label
          >
          <textarea
            :value="runDraft.body"
            class="min-h-[148px] w-full rounded-[16px] border border-slate-200 bg-white px-3 py-3 font-mono text-[12px] leading-6 text-slate-800 outline-none transition focus:border-slate-300 focus:ring-2 focus:ring-slate-100"
            @input="
              handleRunDraftUpdate(
                'body',
                ($event.target as HTMLTextAreaElement).value,
              )
            "
          />
        </div>

        <div v-if="runDraft.triggerMode === 'webhook'" class="space-y-1.5">
          <label
            class="block text-xs font-semibold tracking-wide text-slate-500"
            >Webhook Headers</label
          >
          <textarea
            :value="runDraft.headers"
            class="min-h-28 w-full rounded-2xl border border-slate-200 bg-white px-3 py-3 font-mono text-[12px] leading-6 text-slate-800 outline-none transition focus:border-slate-300 focus:ring-2 focus:ring-slate-100"
            @input="
              handleRunDraftUpdate(
                'headers',
                ($event.target as HTMLTextAreaElement).value,
              )
            "
          />
        </div>

        <div class="space-y-1.5">
          <label
            class="block text-xs font-semibold tracking-wide text-slate-500"
            >Run Env</label
          >
          <textarea
            :value="runDraft.env"
            class="min-h-28 w-full rounded-2xl border border-slate-200 bg-white px-3 py-3 font-mono text-[12px] leading-6 text-slate-800 outline-none transition focus:border-slate-300 focus:ring-2 focus:ring-slate-100"
            @input="
              handleRunDraftUpdate(
                'env',
                ($event.target as HTMLTextAreaElement).value,
              )
            "
          />
        </div>

        <div class="rounded-[18px] border border-slate-200/80 bg-white p-3">
          <div class="flex items-center justify-between text-xs text-slate-500">
            <span class="font-semibold">执行预览</span>
            <span>{{ runnerWorkflowPreview.nodes.length }} nodes</span>
          </div>
          <div class="mt-3 grid grid-cols-2 gap-2 text-xs">
            <div class="rounded-xl bg-slate-50 px-3 py-2">
              <p class="text-slate-400">Trigger</p>
              <p class="mt-1 font-semibold text-slate-700">
                {{ runnerWorkflowPreview.trigger.type }}
              </p>
            </div>
            <div class="rounded-xl bg-slate-50 px-3 py-2">
              <p class="text-slate-400">Transitions</p>
              <p class="mt-1 font-semibold text-slate-700">
                {{ runnerWorkflowPreview.transitions.length }}
              </p>
            </div>
          </div>
        </div>
      </div>

      <div class="shrink-0 border-t border-slate-100 px-4 py-4">
        <Button
          class="h-10 w-full rounded-full bg-slate-900 text-sm font-medium text-white hover:bg-slate-800 disabled:cursor-not-allowed disabled:opacity-60"
          :disabled="isRunningWorkflow"
          @click="handleRunWorkflow"
        >
          <LoaderCircle v-if="isRunningWorkflow" class="h-4 w-4 animate-spin" />
          <Play v-else class="h-4 w-4" />
          {{ runActionLabel }}
        </Button>
        <p class="mt-2 text-[11px] leading-5 text-slate-400">
          运行不会自动发布正式版本，但会把当前画布同步到 Runner
          进行一次最新执行。
        </p>
      </div>
    </aside>

    <!-- Floating Right Properties Panel -->
    <aside
      v-if="isEditMode && selectedNodeId"
      class="pointer-events-auto absolute right-6 top-24 bottom-auto z-10 flex max-h-[calc(100vh-7.5rem)] w-[320px] flex-col overflow-hidden rounded-[20px] bg-white/95 backdrop-blur shadow-sm ring-1 ring-slate-100/50"
    >
      <div
        class="flex h-[68px] shrink-0 items-center gap-3 px-4 border-b border-slate-50"
      >
        <div
          class="flex h-9 w-9 shrink-0 items-center justify-center rounded-[10px] text-white shadow-sm"
          :style="{ backgroundColor: selectedNodeData.accent }"
        >
          <component :is="selectedNodeIcon" class="h-4.5 w-4.5" />
        </div>

        <div class="min-w-0 flex-1 px-1">
          <p class="truncate text-[14px] font-semibold text-slate-900">
            {{ selectedNodeData.subtitle ?? selectedNodeData.title }}
          </p>
          <p class="truncate text-[11px] font-medium text-slate-400">
            {{ selectedNodeData.title }}
          </p>
        </div>

        <button
          type="button"
          class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-600"
          @click="clearSelectedNode"
        >
          <MoreHorizontal class="h-4 w-4" />
        </button>
      </div>

      <Tabs
        class="flex min-h-0 flex-1 flex-col"
        :model-value="activeTab"
        @update:model-value="handleTabChange"
      >
        <div class="px-4 border-b border-slate-50">
          <TabsList class="h-10 w-full bg-slate-50/80 p-1 rounded-lg mt-1 mb-2">
            <TabsTrigger
              v-for="tab in visibleTabs"
              :key="tab"
              :value="tab"
              class="rounded-md px-3 text-xs font-medium data-[state=active]:bg-white data-[state=active]:text-slate-900 data-[state=active]:shadow-sm"
            >
              {{ WORKFLOW_TAB_LABELS[tab] }}
            </TabsTrigger>
          </TabsList>
        </div>

        <div class="min-h-0 flex-1 overflow-y-auto px-4 py-4">
          <TabsContent
            v-for="tab in visibleTabs"
            :key="tab"
            :value="tab"
            class="m-0 h-full"
          >
            <div
              v-if="
                getFieldsForTab(tab).length ||
                (isSelectedSwitchNode && tab === 'mapping')
              "
              class="space-y-4"
            >
              <div
                v-if="isSelectedSwitchNode && tab === 'mapping'"
                class="space-y-3 rounded-[16px] border border-slate-200 bg-slate-50/70 p-3"
              >
                <div class="flex items-center justify-between gap-3">
                  <div>
                    <p
                      class="text-xs font-semibold tracking-wide text-slate-500"
                    >
                      Switch 分支
                    </p>
                    <p class="mt-1 text-[11px] leading-5 text-slate-400">
                      每个分支对应一个独立出口，默认分支会在没有匹配时生效。
                    </p>
                  </div>
                  <button
                    type="button"
                    class="truncate inline-flex h-8 items-center justify-center gap-1 rounded-full border border-slate-200 bg-white px-3 text-xs font-semibold text-slate-700 transition-colors hover:border-slate-300 hover:bg-slate-50"
                    @click="handleAddSwitchBranch"
                  >
                    <Plus class="h-3.5 w-3.5" />
                    添加分支
                  </button>
                </div>

                <div
                  v-for="branch in selectedSwitchBranches"
                  :key="branch.id"
                  class="rounded-[14px] border border-slate-200 bg-white p-3"
                >
                  <div class="flex items-start gap-2">
                    <div class="min-w-0 flex-1 space-y-1.5">
                      <label
                        class="block text-[11px] font-semibold tracking-wide text-slate-500"
                      >
                        分支标签
                      </label>
                      <Input
                        :model-value="branch.label"
                        class="h-9 rounded-lg border-slate-200 bg-white px-3 text-sm shadow-none focus-visible:border-slate-300 focus-visible:ring-2 focus-visible:ring-slate-100"
                        @update:model-value="
                          handleSwitchBranchLabelUpdate(
                            branch.id,
                            String($event),
                          )
                        "
                      />
                      <p
                        class="text-[11px] leading-5 text-slate-400 text-nowrap"
                      >
                        {{
                          `${selectedPanel?.fieldsByTab.base?.find((field) => field.key === "expression")?.value || "value"} === '${branch.label || branch.id}'`
                        }}
                      </p>
                    </div>

                    <button
                      type="button"
                      class="mt-6 inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-full border transition-colors disabled:cursor-not-allowed disabled:opacity-50"
                      :class="
                        selectedSwitchFallbackHandle === branch.id
                          ? 'border-slate-900 bg-slate-900 text-white'
                          : 'border-slate-200 bg-white text-slate-500 hover:border-slate-300 hover:text-slate-800'
                      "
                      :title="
                        selectedSwitchFallbackHandle === branch.id
                          ? '当前默认分支'
                          : '设为默认分支'
                      "
                      @click="handleSwitchFallbackUpdate(branch.id)"
                    >
                      <Check class="h-4 w-4" />
                    </button>

                    <button
                      type="button"
                      class="mt-6 inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-full border border-slate-200 bg-white text-slate-400 transition-colors hover:border-rose-200 hover:text-rose-600 disabled:cursor-not-allowed disabled:opacity-50"
                      :disabled="selectedSwitchBranches.length <= 2"
                      title="删除分支"
                      @click="handleRemoveSwitchBranch(branch.id)"
                    >
                      <Trash2 class="h-4 w-4" />
                    </button>
                  </div>
                </div>
              </div>

              <div
                v-for="field in getFieldsForTab(tab)"
                :key="`${tab}-${field.key}`"
                class="space-y-1.5"
              >
                <label
                  class="block text-xs font-semibold tracking-wide text-slate-500"
                >
                  {{ field.label }}
                </label>

                <Input
                  v-if="field.type === 'input'"
                  :model-value="field.value"
                  class="h-9 rounded-lg border-slate-200 bg-white px-3 text-sm shadow-none focus-visible:border-slate-300 focus-visible:ring-2 focus-visible:ring-slate-100"
                  @update:model-value="
                    handleFieldUpdate(tab, field.key, String($event))
                  "
                />

                <div v-else-if="field.type === 'select'" class="relative">
                  <select
                    :value="field.value"
                    class="h-9 w-full appearance-none rounded-lg border border-slate-200 bg-white px-3 pr-9 text-sm text-slate-800 shadow-none outline-none transition focus:border-slate-300 focus:ring-2 focus:ring-slate-100"
                    @change="
                      handleFieldUpdate(
                        tab,
                        field.key,
                        ($event.target as HTMLSelectElement).value,
                      )
                    "
                  >
                    <option
                      v-for="option in getFieldSelectOptions(field)"
                      :key="`${field.key}-${option.value}`"
                      :value="option.value"
                    >
                      {{ option.label }}
                    </option>
                  </select>
                  <ChevronDown
                    class="pointer-events-none absolute right-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-400"
                  />
                </div>

                <textarea
                  v-else-if="field.type === 'textarea'"
                  :value="field.value"
                  class="min-h-[80px] w-full rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm text-slate-800 shadow-none outline-none transition focus:border-slate-300 focus:ring-2 focus:ring-slate-100"
                  @input="
                    handleFieldUpdate(
                      tab,
                      field.key,
                      ($event.target as HTMLTextAreaElement).value,
                    )
                  "
                />

                <div
                  v-else
                  class="flex h-9 items-center justify-between rounded-lg border px-3 text-sm"
                  :class="
                    field.type === 'readonly'
                      ? 'border-slate-100 bg-slate-50/50 text-slate-500'
                      : 'border-slate-200 bg-white text-slate-800'
                  "
                >
                  <span>{{ field.value }}</span>
                </div>
              </div>
            </div>

            <div
              v-else
              class="flex min-h-[160px] items-center justify-center rounded-xl border border-dashed border-slate-200 bg-slate-50/50 px-6 text-center text-xs leading-5 text-slate-400"
            >
              {{ WORKFLOW_EMPTY_TAB_TEXT[tab] }}
            </div>
          </TabsContent>
        </div>
      </Tabs>
    </aside>

    <aside
      v-else-if="isRunMode"
      class="pointer-events-auto absolute right-6 top-24 bottom-auto z-10 flex max-h-[calc(100vh-7.5rem)] w-90 flex-col overflow-hidden rounded-[20px] bg-white/95 backdrop-blur shadow-sm ring-1 ring-slate-100/50"
    >
      <div
        class="flex h-18 shrink-0 items-center gap-3 border-b border-slate-50 px-4"
      >
        <div
          class="flex h-9.5 w-9.5 items-center justify-center rounded-[12px] bg-slate-900 text-white shadow-sm"
        >
          <Webhook v-if="runDraft.triggerMode === 'webhook'" class="h-4 w-4" />
          <Play v-else class="h-4 w-4" />
        </div>
        <div class="min-w-0 flex-1">
          <p class="truncate text-[14px] font-semibold text-slate-900">
            运行结果
          </p>
          <p class="truncate text-[11px] text-slate-400">
            {{
              activeRunId
                ? `Run ${activeRunId}`
                : "执行后会在这里看到状态和 timeline"
            }}
          </p>
        </div>
        <button
          type="button"
          class="flex h-8 w-8 items-center justify-center rounded-full text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-700 disabled:cursor-not-allowed disabled:opacity-50"
          :disabled="!activeRunId"
          @click="handleRefreshRunSummary"
        >
          <LoaderCircle
            class="h-4 w-4"
            :class="activeRunStatus === 'running' ? 'animate-spin' : ''"
          />
        </button>
      </div>

      <div class="min-h-0 flex-1 space-y-4 overflow-y-auto px-4 py-4">
        <div class="rounded-[18px] border border-slate-200/80 bg-white p-4">
          <div class="flex items-start justify-between gap-3">
            <div>
              <p class="text-xs font-semibold tracking-wide text-slate-500">
                运行状态
              </p>
              <p
                class="mt-2 text-2xl font-semibold tracking-tight text-slate-900"
              >
                {{ activeRunStatusLabel }}
              </p>
            </div>
            <span
              class="rounded-full px-2.5 py-1 text-[11px] font-semibold"
              :class="activeRunStatusClass"
            >
              {{ activeRunStatusLabel }}
            </span>
          </div>

          <button
            type="button"
            class="mt-4 inline-flex h-9 items-center justify-center gap-2 rounded-full border border-rose-200 px-3 text-sm font-medium text-rose-700 transition-colors hover:bg-rose-50 disabled:cursor-not-allowed disabled:border-slate-200 disabled:text-slate-400 disabled:hover:bg-transparent"
            :disabled="!canTerminateActiveRun || isTerminatingWorkflow"
            @click="handleTerminateRun"
          >
            <LoaderCircle
              v-if="isTerminatingWorkflow"
              class="h-4 w-4 animate-spin"
            />
            <Square v-else class="h-4 w-4" />
            {{ isTerminatingWorkflow ? "终止中..." : "终止运行" }}
          </button>

          <div class="mt-4 space-y-2 text-xs text-slate-500">
            <div class="flex items-center justify-between gap-3">
              <span>Workflow ID</span>
              <span class="truncate font-medium text-slate-700">{{
                activeRunWorkflowId || workflowMeta.id
              }}</span>
            </div>
            <div class="flex items-center justify-between gap-3">
              <span>Current Node</span>
              <span class="truncate font-medium text-slate-700">{{
                activeRunSummary?.currentNodeId ?? "--"
              }}</span>
            </div>
            <div class="flex items-center justify-between gap-3">
              <span>Timeline Steps</span>
              <span class="font-medium text-slate-700">{{
                runTimeline.length
              }}</span>
            </div>
          </div>

          <p
            v-if="runErrorMessage"
            class="mt-4 rounded-[14px] bg-rose-50 px-3 py-2 text-xs leading-5 text-rose-700"
          >
            {{ runErrorMessage }}
          </p>
        </div>

        <div class="rounded-[18px] border border-slate-200/80 bg-white p-4">
          <div class="flex items-center justify-between gap-3">
            <p class="text-xs font-semibold tracking-wide text-slate-500">
              执行时间线
            </p>
            <span class="text-[11px] text-slate-400">{{
              runTimeline.length
                ? `${runTimeline.length} steps`
                : "No steps yet"
            }}</span>
          </div>

          <div v-if="runTimeline.length" class="mt-4 space-y-3">
            <article
              v-for="(item, index) in runTimeline"
              :key="`${item.nodeId}-${index}`"
              class="rounded-[16px] border border-slate-200/80 bg-slate-50/70 p-3"
            >
              <div class="flex items-start justify-between gap-3">
                <div class="min-w-0 flex-1">
                  <p class="truncate text-[13px] font-semibold text-slate-800">
                    {{ workflowNodeNameMap[item.nodeId] ?? item.nodeId }}
                  </p>
                  <p class="mt-1 text-[11px] text-slate-400">
                    {{ item.nodeType }} · {{ item.nodeId }}
                  </p>
                </div>
                <span
                  class="rounded-full px-2 py-0.5 text-[11px] font-semibold"
                  :class="
                    item.status === 'success'
                      ? 'bg-emerald-50 text-emerald-700'
                      : item.status === 'waiting'
                        ? 'bg-amber-50 text-amber-700'
                        : item.status === 'failed'
                          ? 'bg-rose-50 text-rose-700'
                          : 'bg-cyan-50 text-cyan-700'
                  "
                >
                  {{ item.status }}
                </span>
              </div>

              <div v-if="item.logs?.length" class="mt-3 space-y-1">
                <p
                  v-for="(log, logIndex) in item.logs"
                  :key="`${item.nodeId}-log-${logIndex}`"
                  class="rounded-xl bg-white px-2.5 py-2 font-mono text-[11px] leading-5 text-slate-500 ring-1 ring-slate-200/80"
                >
                  [{{ log.level }}] {{ log.message }}
                </p>
              </div>
            </article>
          </div>

          <div
            v-else
            class="mt-4 flex min-h-[160px] items-center justify-center rounded-[16px] border border-dashed border-slate-200 bg-slate-50/60 px-6 text-center text-xs leading-5 text-slate-400"
          >
            运行后会按顺序展示每个节点的执行结果和日志。
          </div>
        </div>

        <div class="rounded-[18px] border border-slate-200/80 bg-white p-4">
          <p class="text-xs font-semibold tracking-wide text-slate-500">
            State Snapshot
          </p>
          <pre
            class="mt-3 max-h-[180px] overflow-auto rounded-[14px] bg-slate-950 px-3 py-3 font-mono text-[11px] leading-5 text-slate-100"
            >{{ runStatePreview }}</pre
          >
        </div>

        <div class="rounded-[18px] border border-slate-200/80 bg-white p-4">
          <p class="text-xs font-semibold tracking-wide text-slate-500">
            Last Output
          </p>
          <pre
            class="mt-3 max-h-[180px] overflow-auto rounded-[14px] bg-slate-950 px-3 py-3 font-mono text-[11px] leading-5 text-slate-100"
            >{{ runOutputPreview }}</pre
          >
        </div>
      </div>
    </aside>

    <!-- Floating Bottom Control Toolbar -->
    <div
      v-if="isEditMode"
      class="pointer-events-auto absolute bottom-6 left-1/2 z-20 flex -translate-x-1/2 items-center gap-0.5 rounded-full bg-white p-1 shadow-sm ring-1 ring-slate-100"
    >
      <button
        class="flex h-9 w-10 items-center justify-center rounded-full bg-slate-100 text-slate-700 transition-colors"
      >
        <Hand class="h-4 w-4" />
      </button>
      <button
        class="flex h-9 w-10 items-center justify-center rounded-full text-slate-400 hover:bg-slate-50 hover:text-slate-700 transition-colors"
      >
        <MousePointer2 class="h-4 w-4" />
      </button>
      <div class="mx-1 h-4 w-px bg-slate-100"></div>
      <button
        class="flex h-9 w-10 items-center justify-center rounded-full text-slate-400 hover:bg-slate-50 hover:text-slate-700 transition-colors"
        @click="undoLastChange"
      >
        <Undo2 class="h-4 w-4" />
      </button>
      <button
        class="flex h-9 w-10 items-center justify-center rounded-full text-slate-400 hover:bg-slate-50 hover:text-slate-700 transition-colors"
      >
        <Redo2 class="h-4 w-4" />
      </button>
    </div>

    <div
      v-if="isLoadingWorkflow"
      class="absolute inset-0 z-30 flex items-center justify-center bg-white/55 backdrop-blur-[2px]"
    >
      <div
        class="rounded-full border border-slate-200 bg-white px-4 py-2 text-sm font-medium text-slate-600 shadow-sm"
      >
        Loading workflow...
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  reactive,
  ref,
  watch,
} from "vue";
import {
  type Connection,
  type Edge,
  VueFlow,
  useVueFlow,
} from "@vue-flow/core";
import {
  Check,
  ChevronDown,
  ChevronLeft,
  Compass,
  Code,
  Bot,
  Hand,
  LoaderCircle,
  MoreHorizontal,
  MousePointer2,
  Pencil,
  Play,
  Plus,
  Redo2,
  Settings,
  Square,
  Trash2,
  Undo2,
  Webhook,
} from "lucide-vue-next";
import { type LocationQueryValue, useRoute, useRouter } from "vue-router";
import { toast } from "vue-sonner";

import WorkflowBranchChipNode from "@/components/workflow/WorkflowBranchChipNode.vue";
import WorkflowCanvasNode from "@/components/workflow/WorkflowCanvasNode.vue";
import WorkflowRunListDialog from "@/components/workflow/WorkflowRunListDialog.vue";
import WorkflowTerminalNode from "@/components/workflow/WorkflowTerminalNode.vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  fetchWorkflowDetail,
  type WorkflowDetail,
} from "@/features/workflow/api";
import { createWorkflowExportDocument } from "@/features/workflow/export";
import { createWorkflowEditorStateFromRunnerDefinition } from "@/features/workflow/import";
import {
  createInitialWorkflowEditorState,
  createNewWorkflowEditorState,
  createPersistedWorkflowDocument,
  createWorkflowEditorStateFromDocument,
  type WorkflowPageMode,
  type WorkflowRunDraft,
  type WorkflowEditorState,
} from "@/features/workflow/persistence";
import {
  buildRunnerWorkflowDefinition,
  executeWorkflowRun,
  fetchWorkflowRunSummary,
  publishWorkflowToRunner,
  shouldPollWorkflowRunSummary,
  syncWorkflowToRunner,
  terminateWorkflowRun,
  type RunnerWorkflowDefinition,
  type WorkflowRunStatus,
  type WorkflowRunSummary,
} from "@/features/workflow/runner";
import {
  WORKFLOW_EDIT_SESSION_RUNNER_BASE_URL,
  buildWorkflowEditSessionWsUrl,
  createWorkflowEditSession,
  type WorkflowEditSession,
  type WorkflowEditSessionEvent,
} from "@/features/workflow/session";
import {
  WORKFLOW_EMPTY_TAB_TEXT,
  WORKFLOW_EDGE_STYLE,
  WORKFLOW_EDGE_TYPE,
  WORKFLOW_ICON_MAP,
  WORKFLOW_PALETTE_CATEGORIES,
  WORKFLOW_TAB_LABELS,
  createSwitchBranchHandleId,
  createWorkflowNodeDraft,
  getWorkflowFieldSelectOptions,
  getSwitchBranches,
  getSwitchFallbackHandle,
  setSwitchBranches,
  setSwitchFallbackHandle,
  syncBranchHandlesForNode,
  type WorkflowExecutionStatus,
  type WorkflowField,
  type WorkflowFlowNode,
  type WorkflowIconKey,
  type WorkflowNodeData,
  type WorkflowNodeKind,
  type WorkflowNodePanel,
  type WorkflowPaletteItem,
  type WorkflowTabId,
} from "@/features/workflow/model";

const DRAG_DATA_TYPE = "application/x-ses-workflow-node";
const HISTORY_LIMIT = 50;
const DEFAULT_WORKFLOW_ID = "sorting-main-flow";

const route = useRoute();
const router = useRouter();
const initialEditorState = createInitialWorkflowEditorState();
const nodes = ref<WorkflowFlowNode[]>(initialEditorState.nodes);
const edges = ref<Edge[]>(initialEditorState.edges);
const panelByNodeId = ref<Record<string, WorkflowNodePanel>>(
  initialEditorState.panelByNodeId,
);
const searchQuery = ref("");
const selectedNodeId = ref(initialEditorState.selectedNodeId);
const activeTab = ref<WorkflowTabId>(initialEditorState.activeTab);
const pageMode = ref<WorkflowPageMode>(initialEditorState.pageMode);
const runDraft = ref<WorkflowRunDraft>(initialEditorState.runDraft);
const activeDragPaletteItemId = ref<string | null>(null);
const isCanvasDropTarget = ref(false);
const isPublishing = ref(false);
const isLoadingWorkflow = ref(false);
const isRunningWorkflow = ref(false);
const isWorkflowRunListOpen = ref(false);
const isTerminatingWorkflow = ref(false);
const isCreatingAssistantSession = ref(false);
const assistantSession = ref<WorkflowEditSession | null>(null);
const assistantSessionError = ref("");
const assistantConnectionState = ref<
  "idle" | "connecting" | "connected" | "disconnected"
>("idle");
const historyStack = ref<WorkflowEditorSnapshot[]>([]);
const activeRunSummary = ref<WorkflowRunSummary | null>(null);
const activeRunId = ref("");
const activeRunWorkflowId = ref("");
const runErrorMessage = ref("");
let runSummaryPollTimer: number | null = null;
let assistantSessionSocket: WebSocket | null = null;
const getRouteWorkflowId = (value: string | string[] | undefined) => {
  const routeValue = Array.isArray(value) ? value[0] : value;
  const normalizedValue = routeValue?.trim();

  return normalizedValue || DEFAULT_WORKFLOW_ID;
};

const workflowMeta = reactive({
  id: getRouteWorkflowId(route.params.id as string | string[] | undefined),
  name: "sorting-main-flow",
  status: "draft" as "draft" | "published",
  version: "v3",
});
const expandedCategories = reactive<Record<string, boolean>>(
  Object.fromEntries(
    WORKFLOW_PALETTE_CATEGORIES.map((category) => [
      category.id,
      category.defaultOpen,
    ]),
  ),
);
const { fitView, onPaneReady, screenToFlowCoordinate } = useVueFlow();
const isCanvasPaneReady = ref(false);
const shouldResetCanvasViewport = ref(false);

const canvasTools = [
  { id: "select", icon: "mousePointer" as WorkflowIconKey },
  { id: "pan", icon: "hand" as WorkflowIconKey },
  { id: "fit", icon: "maximize" as WorkflowIconKey },
  { id: "lock", icon: "lock" as WorkflowIconKey },
];

const EMPTY_NODE_DATA: WorkflowNodeData = {
  accent: "#3B82F6",
  executionStatus: undefined,
  icon: "database",
  kind: "fetch",
  nodeKey: "unselected",
  subtitle: "请选择节点",
  title: "未选择节点",
};

interface WorkflowEditorSnapshot {
  activeTab: WorkflowTabId;
  edges: Edge[];
  nodes: WorkflowFlowNode[];
  panelByNodeId: Record<string, WorkflowNodePanel>;
  selectedNodeId: string;
}

const selectedNodeData = ref<WorkflowNodeData>(EMPTY_NODE_DATA);
const selectedPanel = computed(() => panelByNodeId.value[selectedNodeId.value]);
const visibleTabs = computed(() => selectedPanel.value?.tabs ?? ["base"]);
const selectedNodeIcon = computed(
  () => WORKFLOW_ICON_MAP[selectedNodeData.value.icon],
);
const isEditMode = computed(() => pageMode.value === "edit");
const isRunMode = computed(() => pageMode.value === "run");
const isAiMode = computed(() => pageMode.value === "ai");
const isSelectedSwitchNode = computed(
  () => selectedNodeData.value.kind === "switch",
);
const selectedSwitchBranches = computed(() =>
  getSwitchBranches(selectedPanel.value),
);
const selectedSwitchFallbackHandle = computed(
  () => getSwitchFallbackHandle(selectedPanel.value) ?? "",
);
const workflowStatusLabel = computed(() =>
  workflowMeta.status === "published" ? "Published" : "Draft",
);
const publishButtonLabel = computed(() =>
  isPublishing.value ? "Publishing..." : "Publish",
);
const runActionLabel = computed(() =>
  isRunningWorkflow.value
    ? "运行中..."
    : activeRunId.value
      ? "重新运行"
      : "运行当前工作流",
);
const canTerminateActiveRun = computed(
  () =>
    Boolean(activeRunId.value) &&
    (activeRunStatus.value === "running" ||
      activeRunStatus.value === "waiting"),
);
const persistedWorkflowId = computed(() =>
  route.name === "workflow-editor" && typeof route.params.id === "string"
    ? route.params.id
    : undefined,
);
const workflowTitle = computed(() => workflowMeta.name || "New workflow");
const hasAssistantSession = computed(() => Boolean(assistantSession.value));
const assistantRunnerBaseUrl = WORKFLOW_EDIT_SESSION_RUNNER_BASE_URL;
const assistantSessionId = computed(
  () => assistantSession.value?.sessionId ?? "",
);
const assistantConnectionLabel = computed(() => {
  switch (assistantConnectionState.value) {
    case "connecting":
      return "连接中";
    case "connected":
      return "已连接";
    case "disconnected":
      return "已断开";
    default:
      return "未启动";
  }
});
const assistantSessionUpdatedLabel = computed(() => {
  if (!assistantSession.value?.updatedAt) {
    return "尚未同步";
  }

  return new Date(assistantSession.value.updatedAt).toLocaleString("zh-CN");
});
const workflowNodeNameMap = computed<Record<string, string>>(() =>
  nodes.value.reduce<Record<string, string>>((accumulator, node) => {
    accumulator[node.id] = node.data.subtitle ?? node.data.title;
    return accumulator;
  }, {}),
);
const getRunnerWorkflowPreview = (): RunnerWorkflowDefinition =>
  buildRunnerWorkflowDefinition(nodes.value, edges.value, panelByNodeId.value, {
    workflowId: workflowMeta.id,
    workflowName: workflowMeta.name,
    workflowVersion: workflowMeta.version,
    workflowStatus: workflowMeta.status,
  });
const runnerWorkflowPreview = computed<RunnerWorkflowDefinition>(
  getRunnerWorkflowPreview,
);
const runnerTriggerSummaryLabel = computed(() => {
  if (runnerWorkflowPreview.value.trigger.type === "webhook") {
    return runnerWorkflowPreview.value.trigger.responseMode === "sync"
      ? "Webhook Trigger · Sync"
      : "Webhook Trigger · Async Ack";
  }

  return "Manual Trigger";
});
const activeRunStatus = computed<WorkflowRunStatus | null>(
  () => activeRunSummary.value?.status ?? null,
);
const activeRunStatusLabel = computed(() => {
  switch (activeRunStatus.value) {
    case "running":
      return "运行中";
    case "completed":
      return "已完成";
    case "waiting":
      return "等待恢复";
    case "failed":
      return "失败";
    case "terminated":
      return "已终止";
    default:
      return "未运行";
  }
});
const activeRunStatusClass = computed(() => {
  switch (activeRunStatus.value) {
    case "running":
      return "bg-cyan-50 text-cyan-700";
    case "completed":
      return "bg-emerald-50 text-emerald-700";
    case "waiting":
      return "bg-amber-50 text-amber-700";
    case "failed":
      return "bg-rose-50 text-rose-700";
    case "terminated":
      return "bg-slate-200 text-slate-700";
    default:
      return "bg-slate-100 text-slate-500";
  }
});
const runTimeline = computed(() => activeRunSummary.value?.timeline ?? []);
const runStatePreview = computed(() =>
  formatJsonPreview(activeRunSummary.value?.state ?? {}),
);
const runOutputPreview = computed(() => {
  const lastItem = runTimeline.value[runTimeline.value.length - 1];
  return formatJsonPreview(lastItem?.output ?? {});
});

const filteredCategories = computed(() => {
  const keyword = searchQuery.value.trim().toLowerCase();

  return WORKFLOW_PALETTE_CATEGORIES.map((category) => ({
    ...category,
    items: keyword
      ? category.items.filter((item) =>
          item.label.toLowerCase().includes(keyword),
        )
      : category.items,
  })).filter((category) => category.items.length > 0);
});

const paletteItemMap = computed<Record<string, WorkflowPaletteItem>>(() =>
  WORKFLOW_PALETTE_CATEGORIES.flatMap((category) => category.items).reduce<
    Record<string, WorkflowPaletteItem>
  >((acc, item) => {
    acc[item.id] = item;
    return acc;
  }, {}),
);

watch(
  visibleTabs,
  (tabs) => {
    if (!tabs.includes(activeTab.value)) {
      activeTab.value = tabs[0];
    }
  },
  { immediate: true },
);

const resolveIcon = (icon: WorkflowIconKey) => WORKFLOW_ICON_MAP[icon];

const isSwitchBranchField = (fieldKey: string) =>
  fieldKey.startsWith("branch:");

const getFieldsForTab = (tab: WorkflowTabId) => {
  const fields = selectedPanel.value?.fieldsByTab[tab] ?? [];

  if (!isSelectedSwitchNode.value) {
    return fields;
  }

  return fields.filter((field) => {
    if (tab === "base" && field.key === "fallback") {
      return false;
    }

    if (tab === "mapping" && isSwitchBranchField(field.key)) {
      return false;
    }

    return true;
  });
};

const getFieldSelectOptions = (field: WorkflowField) =>
  getWorkflowFieldSelectOptions(selectedPanel.value, field);

const syncBranchHandleNodes = (nodeId?: string) => {
  nodes.value = nodes.value.map((node) => {
    if (node.data.kind !== "switch" && node.data.kind !== "if-else") {
      return node;
    }

    if (nodeId && node.id !== nodeId) {
      return node;
    }

    return syncBranchHandlesForNode(node, panelByNodeId.value[node.id]);
  }) as WorkflowFlowNode[];
  syncSelectedNodeData();
};

const getNextSwitchBranchHandleId = (panel: WorkflowNodePanel) => {
  const existingHandleIds = new Set(
    getSwitchBranches(panel).map((branch) => branch.id),
  );
  let index = existingHandleIds.size;
  let nextHandleId = createSwitchBranchHandleId(index);

  while (existingHandleIds.has(nextHandleId)) {
    index += 1;
    nextHandleId = createSwitchBranchHandleId(index);
  }

  return nextHandleId;
};

const getNextSwitchBranchLabel = (panel: WorkflowNodePanel) => {
  const existingLabels = new Set(
    getSwitchBranches(panel).map((branch) => branch.label),
  );
  let index = existingLabels.size;
  let nextLabel =
    index < 26 ? String.fromCharCode(65 + index) : `Branch ${index + 1}`;

  while (existingLabels.has(nextLabel)) {
    index += 1;
    nextLabel =
      index < 26 ? String.fromCharCode(65 + index) : `Branch ${index + 1}`;
  }

  return nextLabel;
};

const handleBackToList = () => {
  void router.push({ name: "workflow-list" });
};

const handleOpenWorkflowRuns = () => {
  if (!persistedWorkflowId.value) {
    return;
  }

  isWorkflowRunListOpen.value = true;
};

const ensureAssistantSessionForAiMode = async () => {
  if (isCreatingAssistantSession.value) {
    return;
  }

  if (assistantSession.value?.sessionId) {
    if (!assistantSessionSocket) {
      connectAssistantSessionSocket(assistantSession.value.sessionId);
    }

    return;
  }

  isCreatingAssistantSession.value = true;
  assistantSessionError.value = "";

  try {
    const session = await createWorkflowEditSession({
      editorDocument: buildCurrentEditorDocument({
        pageMode: "ai",
      }),
      workflow: runnerWorkflowPreview.value,
      workflowId: persistedWorkflowId.value,
    });

    assistantSession.value = session;
    connectAssistantSessionSocket(session.sessionId);
    toast.success(`AI 编辑会话已创建：${session.sessionId}`);
  } catch (error) {
    assistantSessionError.value =
      error instanceof Error ? error.message : "创建 AI 编辑会话失败";
    toast.error(assistantSessionError.value);
  } finally {
    isCreatingAssistantSession.value = false;
  }
};

const handleWorkflowRunListOpenChange = (open: boolean) => {
  isWorkflowRunListOpen.value = open;
};

const handleOpenWorkflowRunFromList = (runId: string) => {
  if (!persistedWorkflowId.value) {
    return;
  }

  void router.push({
    name: "workflow-editor",
    params: {
      id: persistedWorkflowId.value,
    },
    query: {
      runId,
    },
  });
};

const resetCanvasViewport = async () => {
  await nextTick();
  await fitView({
    padding: 0.2,
  });
};

const queueCanvasViewportReset = () => {
  if (!isCanvasPaneReady.value) {
    shouldResetCanvasViewport.value = true;
    return;
  }

  void resetCanvasViewport();
};

const buildCurrentEditorDocument = (
  overrides: Partial<{
    pageMode: WorkflowPageMode;
    status: "draft" | "published";
  }> = {},
) =>
  createPersistedWorkflowDocument(
    nodes.value,
    edges.value,
    panelByNodeId.value,
    {
      activeTab: activeTab.value,
      pageMode: overrides.pageMode ?? pageMode.value,
      runDraft: runDraft.value,
      selectedNodeId: selectedNodeId.value,
      status: overrides.status ?? workflowMeta.status,
      version: workflowMeta.version,
      workflowId: workflowMeta.id,
      workflowName: workflowMeta.name,
    },
  );

const applyAssistantSessionPreview = (session: WorkflowEditSession) => {
  const nextState = session.editorDocument
    ? createWorkflowEditorStateFromDocument(session.editorDocument)
    : createWorkflowEditorStateFromRunnerDefinition(session.workflow);

  workflowMeta.id = session.workflowId?.trim() || workflowMeta.id;
  workflowMeta.name = session.workflow.meta.name ?? workflowMeta.name;
  workflowMeta.status =
    session.workflow.meta.status === "published" ? "published" : "draft";
  workflowMeta.version = `v${session.workflow.meta.version}`;
  applyWorkflowEditorState(nextState);
  pageMode.value = "ai";
};

const closeAssistantSessionSocket = () => {
  if (assistantSessionSocket) {
    const socket = assistantSessionSocket;
    assistantSessionSocket = null;
    assistantConnectionState.value = "idle";
    socket.close();
  }
};

const connectAssistantSessionSocket = (sessionId: string) => {
  closeAssistantSessionSocket();
  assistantConnectionState.value = "connecting";

  const socket = new WebSocket(buildWorkflowEditSessionWsUrl(sessionId));
  assistantSessionSocket = socket;

  socket.onopen = () => {
    assistantConnectionState.value = "connected";
    assistantSessionError.value = "";
  };

  socket.onmessage = (event) => {
    try {
      const payload = JSON.parse(event.data) as WorkflowEditSessionEvent;

      if (!payload.session) {
        return;
      }

      assistantSession.value = payload.session;
      applyAssistantSessionPreview(payload.session);
    } catch {
      assistantSessionError.value = "AI 会话更新解析失败";
    }
  };

  socket.onerror = () => {
    assistantSessionError.value = "AI 会话连接异常";
  };

  socket.onclose = () => {
    if (assistantSessionSocket === socket) {
      assistantSessionSocket = null;
    }

    assistantConnectionState.value = hasAssistantSession.value
      ? "disconnected"
      : "idle";
  };
};

const resetAssistantSession = () => {
  closeAssistantSessionSocket();
  assistantSession.value = null;
  assistantSessionError.value = "";
  assistantConnectionState.value = "idle";
};

const applyWorkflowEditorState = (state: WorkflowEditorState) => {
  nodes.value = state.nodes;
  edges.value = state.edges;
  panelByNodeId.value = state.panelByNodeId;
  activeTab.value = state.activeTab;
  pageMode.value = state.pageMode;
  runDraft.value = { ...state.runDraft };
  historyStack.value = [];
  syncBranchHandleNodes();
  setSelectedNode(state.selectedNodeId);

  if (activeRunSummary.value && activeRunWorkflowId.value === workflowMeta.id) {
    setNodeExecutionStatuses(activeRunSummary.value);
  }

  queueCanvasViewportReset();
};

const updateNodeExecutionStatus = (
  node: WorkflowFlowNode,
  statusByNodeId: Map<string, WorkflowExecutionStatus>,
): WorkflowFlowNode => {
  const newNode = { ...node };
  newNode.data = {
    ...node.data,
    executionStatus:
      node.data.kind === "branch-label"
        ? undefined
        : statusByNodeId.get(node.id),
  };
  return newNode;
};

const setNodeExecutionStatuses = (summary: WorkflowRunSummary | null) => {
  const statusByNodeId = new Map<string, WorkflowExecutionStatus>();

  if (summary) {
    summary.timeline.forEach((item) => {
      statusByNodeId.set(item.nodeId, item.status as WorkflowExecutionStatus);
    });

    if (summary.status === "running" && summary.currentNodeId) {
      statusByNodeId.set(summary.currentNodeId, "running");
    }
  }

  nodes.value = nodes.value.map((node) =>
    updateNodeExecutionStatus(node, statusByNodeId),
  );
  syncSelectedNodeData();
};

const resetRunSession = () => {
  clearRunSummaryPolling();
  activeRunSummary.value = null;
  activeRunId.value = "";
  activeRunWorkflowId.value = "";
  isTerminatingWorkflow.value = false;
  runErrorMessage.value = "";
  setNodeExecutionStatuses(null);
};

const resetToInitialWorkflow = () => {
  const nextState = createNewWorkflowEditorState();

  workflowMeta.id = DEFAULT_WORKFLOW_ID;
  workflowMeta.name = DEFAULT_WORKFLOW_ID;
  workflowMeta.status = "draft";
  workflowMeta.version = "v3";
  resetAssistantSession();
  resetRunSession();
  applyWorkflowEditorState(nextState);
};

const getRouteRunId = (
  value: LocationQueryValue | LocationQueryValue[] | undefined,
) => {
  const normalizedValue = Array.isArray(value) ? value[0] : value;

  return typeof normalizedValue === "string" && normalizedValue.trim()
    ? normalizedValue.trim()
    : "";
};

const clearRouteRunId = async (workflowId: string) => {
  if (route.name !== "workflow-editor") {
    return;
  }

  const { runId: _ignoredRunId, ...restQuery } = route.query;

  await router.replace({
    name: "workflow-editor",
    params: {
      id: workflowId,
    },
    query: restQuery,
  });
};

const restoreWorkflowRunFromRoute = async (
  workflowId: string,
  workflow: WorkflowDetail,
  requestedRunId: string,
) => {
  try {
    const summary = await fetchWorkflowRunSummary(requestedRunId);

    if (
      summary.workflowKey !== workflow.workflow.meta.key ||
      summary.workflowVersion !== workflow.workflow.meta.version
    ) {
      resetRunSession();
      toast.error("该运行记录不属于当前工作流");
      await clearRouteRunId(workflowId);
      return;
    }

    activeRunWorkflowId.value = workflowId;
    activeRunId.value = summary.runId;
    activeRunSummary.value = summary;
    pageMode.value = "run";
    runErrorMessage.value = "";
    isTerminatingWorkflow.value = false;
    setNodeExecutionStatuses(summary);
    selectRunFocusedNode(summary);

    if (shouldPollWorkflowRunSummary(summary.status)) {
      await refreshRunSummary();
      return;
    }

    clearRunSummaryPolling();
  } catch (error) {
    resetRunSession();
    runErrorMessage.value =
      error instanceof Error ? error.message : "加载运行状态失败";
    toast.error(runErrorMessage.value);
    await clearRouteRunId(workflowId);
  }
};

const loadWorkflowDetail = async (workflowId: string, requestedRunId = "") => {
  isLoadingWorkflow.value = true;

  if (
    assistantSession.value?.workflowId &&
    assistantSession.value.workflowId !== workflowId
  ) {
    resetAssistantSession();
  }

  try {
    const workflow = await fetchWorkflowDetail(workflowId);
    const state = workflow.document
      ? createWorkflowEditorStateFromDocument(workflow.document)
      : createWorkflowEditorStateFromRunnerDefinition(workflow.workflow);

    if (activeRunWorkflowId.value && activeRunWorkflowId.value !== workflowId) {
      resetRunSession();
    }

    workflowMeta.id = workflow.workflowId;
    workflowMeta.name = workflow.name;
    workflowMeta.status = workflow.status;
    workflowMeta.version = workflow.version;
    applyWorkflowEditorState(state);

    if (requestedRunId) {
      await restoreWorkflowRunFromRoute(workflowId, workflow, requestedRunId);
      return;
    }

    pageMode.value = "edit";

    clearRunSummaryPolling();

    if (activeRunSummary.value && activeRunWorkflowId.value === workflowId) {
      setNodeExecutionStatuses(activeRunSummary.value);
      selectRunFocusedNode(activeRunSummary.value);
    }
  } catch (error) {
    toast.error(error instanceof Error ? error.message : "加载工作流详情失败");
    void router.replace({ name: "workflow-list" });
  } finally {
    isLoadingWorkflow.value = false;
  }
};

const handleTabChange = (value: string | number) => {
  if (
    typeof value === "string" &&
    visibleTabs.value.includes(value as WorkflowTabId)
  ) {
    activeTab.value = value as WorkflowTabId;
  }
};

const handlePageModeChange = (mode: WorkflowPageMode) => {
  if (mode === "edit") {
    resetRunSession();
    closeAssistantSessionSocket();

    if (getRouteRunId(route.query.runId)) {
      void clearRouteRunId(workflowMeta.id);
    }
  }

  if (mode === "run") {
    closeAssistantSessionSocket();
  }

  pageMode.value = mode;

  if (mode === "ai") {
    resetRunSession();
    void ensureAssistantSessionForAiMode();
  }
};

const syncSelectedNodeData = () => {
  selectedNodeData.value =
    nodes.value.find((node) => node.id === selectedNodeId.value)?.data ??
    EMPTY_NODE_DATA;
};

watch(
  [() => route.name, () => route.params.id],
  async ([routeName, routeWorkflowId]) => {
    if (
      routeName === "workflow-editor" &&
      typeof routeWorkflowId === "string" &&
      routeWorkflowId.trim()
    ) {
      await loadWorkflowDetail(
        routeWorkflowId,
        getRouteRunId(route.query.runId),
      );
      return;
    }

    resetToInitialWorkflow();
  },
  { immediate: true },
);

const cloneWorkflowNodeData = (data: WorkflowNodeData): WorkflowNodeData => ({
  active: data.active,
  accent: data.accent,
  branchHandles: data.branchHandles?.map((branch) => ({
    id: branch.id,
    isDefault: branch.isDefault,
    label: branch.label,
  })),
  executionStatus: data.executionStatus,
  icon: data.icon,
  kind: data.kind,
  nodeKey: data.nodeKey,
  status: data.status,
  subtitle: data.subtitle,
  title: data.title,
});

const cloneWorkflowNodes = (sourceNodes: WorkflowFlowNode[]) =>
  sourceNodes.map<WorkflowFlowNode>((node) => ({
    data: cloneWorkflowNodeData(node.data),
    deletable: node.deletable,
    draggable: node.draggable,
    id: node.id,
    parentNode: node.parentNode,
    position: {
      x: node.position.x,
      y: node.position.y,
    },
    selectable: node.selectable,
    sourcePosition: node.sourcePosition,
    targetPosition: node.targetPosition,
    type: node.type,
  }));

const cloneEdgeStyle = (style: Edge["style"]) => {
  if (!style || typeof style !== "object" || Array.isArray(style)) {
    return undefined;
  }

  return Object.entries(style).reduce<Record<string, string | number>>(
    (acc, [key, value]) => {
      if (typeof value === "string" || typeof value === "number") {
        acc[key] = value;
      }

      return acc;
    },
    {},
  );
};

const cloneWorkflowEdges = (sourceEdges: Edge[]) =>
  sourceEdges.map<Edge>((edge) => ({
    animated: edge.animated,
    deletable: edge.deletable,
    id: edge.id,
    interactionWidth: edge.interactionWidth,
    selectable: edge.selectable,
    source: edge.source,
    sourceHandle: edge.sourceHandle,
    style: cloneEdgeStyle(edge.style),
    target: edge.target,
    targetHandle: edge.targetHandle,
    type: edge.type,
    updatable: edge.updatable,
  }));

const cloneWorkflowPanels = (sourcePanels: Record<string, WorkflowNodePanel>) =>
  Object.fromEntries(
    Object.entries(sourcePanels).map(([nodeId, panel]) => [
      nodeId,
      {
        fieldsByTab: Object.fromEntries(
          Object.entries(panel.fieldsByTab).map(([tab, fields]) => [
            tab,
            (fields ?? []).map((field) => ({
              key: field.key,
              label: field.label,
              type: field.type,
              value: field.value,
            })),
          ]),
        ),
        tabs: [...panel.tabs],
      } satisfies WorkflowNodePanel,
    ]),
  ) as Record<string, WorkflowNodePanel>;

const createSnapshot = (): WorkflowEditorSnapshot => ({
  activeTab: activeTab.value,
  edges: cloneWorkflowEdges(edges.value),
  nodes: cloneWorkflowNodes(nodes.value),
  panelByNodeId: cloneWorkflowPanels(panelByNodeId.value),
  selectedNodeId: selectedNodeId.value,
});

const pushHistorySnapshot = () => {
  historyStack.value = [
    ...historyStack.value.slice(-(HISTORY_LIMIT - 1)),
    createSnapshot(),
  ];
};

const restoreSnapshot = (snapshot: WorkflowEditorSnapshot) => {
  nodes.value = cloneWorkflowNodes(snapshot.nodes);
  edges.value = cloneWorkflowEdges(snapshot.edges);
  panelByNodeId.value = cloneWorkflowPanels(snapshot.panelByNodeId);
  selectedNodeId.value = snapshot.selectedNodeId;
  activeTab.value = snapshot.activeTab;
  syncBranchHandleNodes();
  syncSelectedNodeData();
};

const undoLastChange = () => {
  const snapshot = historyStack.value[historyStack.value.length - 1];

  if (!snapshot) {
    toast.info("没有可撤销的操作");
    return;
  }

  historyStack.value = historyStack.value.slice(0, -1);
  restoreSnapshot(snapshot);
  toast.success("已撤销上一步操作");
};

const clearSelectedNode = () => {
  selectedNodeId.value = "";
  nodes.value = nodes.value.map((node) => ({
    ...node,
    selected: false,
    data: {
      ...node.data,
      active: false,
    },
  })) as WorkflowFlowNode[];
  syncSelectedNodeData();
};

const selectFallbackNode = () => {
  const fallbackNode = nodes.value.find((node) => node.type !== "branch-chip");

  if (fallbackNode) {
    setSelectedNode(fallbackNode.id);
    return;
  }

  clearSelectedNode();
};

const setSelectedNode = (nodeId: string) => {
  selectedNodeId.value = nodeId;
  nodes.value = nodes.value.map((node) => ({
    ...node,
    selected: node.id === nodeId,
    data: {
      ...node.data,
      active: node.id === nodeId,
    },
  })) as WorkflowFlowNode[];
  syncSelectedNodeData();
};

const handleNodeClick = (payload: any) => {
  setSelectedNode(payload.node.id);
};

const handlePaneClick = () => {
  if (!isEditMode.value) {
    return;
  }

  clearSelectedNode();
};

const deleteSelectedNode = () => {
  if (!isEditMode.value) {
    return;
  }

  const targetId = selectedNodeId.value;

  if (!targetId) {
    return;
  }

  const targetNode = nodes.value.find((node) => node.id === targetId);

  if (!targetNode || targetNode.type === "branch-chip") {
    return;
  }

  pushHistorySnapshot();
  nodes.value = nodes.value.filter((node) => node.id !== targetId);
  edges.value = edges.value.filter(
    (edge) => edge.source !== targetId && edge.target !== targetId,
  );

  const { [targetId]: _removedPanel, ...restPanels } = panelByNodeId.value;
  panelByNodeId.value = restPanels;

  selectFallbackNode();
  toast.success(
    `已删除节点：${targetNode.data.subtitle ?? targetNode.data.title}`,
  );
};

const getEdgeId = (connection: Connection) => {
  const sourceHandle = connection.sourceHandle ?? "default";
  const targetHandle = connection.targetHandle ?? "default";

  return `edge:${connection.source}:${sourceHandle}->${connection.target}:${targetHandle}`;
};

const handleConnect = (connection: Connection) => {
  if (!isEditMode.value) {
    return;
  }

  if (!connection.source || !connection.target) {
    return;
  }

  const nextEdgeId = getEdgeId(connection);

  if (edges.value.some((edge) => edge.id === nextEdgeId)) {
    toast.info("这条连线已经存在");
    return;
  }

  pushHistorySnapshot();
  edges.value = [
    ...edges.value,
    {
      id: nextEdgeId,
      source: connection.source,
      sourceHandle: connection.sourceHandle,
      target: connection.target,
      targetHandle: connection.targetHandle,
      type: WORKFLOW_EDGE_TYPE,
      style: WORKFLOW_EDGE_STYLE,
    },
  ];

  toast.success("已创建连线");
};

const toggleCategory = (categoryId: string) => {
  expandedCategories[categoryId] = !expandedCategories[categoryId];
};

const isCategoryOpen = (categoryId: string) => {
  if (searchQuery.value.trim()) {
    return true;
  }

  return expandedCategories[categoryId];
};

const focusPaletteItem = (kind: WorkflowNodeKind) => {
  if (!isEditMode.value) {
    return;
  }

  const targetNode = nodes.value.find(
    (node) => node.data.kind === kind && node.type !== "branch-chip",
  );

  if (targetNode) {
    setSelectedNode(targetNode.id);
  }
};

const handlePaletteDragStart = (event: DragEvent, itemId: string) => {
  if (!isEditMode.value || !event.dataTransfer) {
    return;
  }

  activeDragPaletteItemId.value = itemId;
  event.dataTransfer.effectAllowed = "copy";
  event.dataTransfer.setData(DRAG_DATA_TYPE, itemId);
  event.dataTransfer.setData("text/plain", itemId);
};

const handlePaletteDragEnd = () => {
  activeDragPaletteItemId.value = null;
  isCanvasDropTarget.value = false;
};

const handleCanvasDragEnter = (event: DragEvent) => {
  if (!isEditMode.value) {
    return;
  }

  if (!event.dataTransfer?.types.includes(DRAG_DATA_TYPE)) {
    return;
  }

  isCanvasDropTarget.value = true;
};

const handleCanvasDragOver = (event: DragEvent) => {
  if (!isEditMode.value) {
    return;
  }

  if (!event.dataTransfer?.types.includes(DRAG_DATA_TYPE)) {
    return;
  }

  event.dataTransfer.dropEffect = "copy";
  isCanvasDropTarget.value = true;
};

const handleCanvasDrop = (event: DragEvent) => {
  if (!isEditMode.value) {
    return;
  }

  isCanvasDropTarget.value = false;

  const itemId = event.dataTransfer?.getData(DRAG_DATA_TYPE);
  const item = itemId ? paletteItemMap.value[itemId] : undefined;

  activeDragPaletteItemId.value = null;

  if (!item) {
    return;
  }

  const flowPosition = screenToFlowCoordinate({
    x: event.clientX,
    y: event.clientY,
  });
  const { node, panel } = createWorkflowNodeDraft(
    item,
    {
      x: Math.max(
        24,
        flowPosition.x -
          (item.id === "palette-start" || item.id === "palette-end" ? 32 : 110),
      ),
      y: Math.max(
        24,
        flowPosition.y -
          (item.id === "palette-start" || item.id === "palette-end" ? 32 : 36),
      ),
    },
    nodes.value,
  );

  pushHistorySnapshot();
  nodes.value = [...nodes.value, node];
  panelByNodeId.value = {
    ...panelByNodeId.value,
    [node.id]: panel,
  };
  setSelectedNode(node.id);

  toast.success(`已添加节点：${node.data.subtitle ?? node.data.title}`);
};

const handleFieldUpdate = (
  tab: WorkflowTabId,
  fieldKey: string,
  value: string,
) => {
  if (!isEditMode.value) {
    return;
  }

  const panel = selectedPanel.value;

  if (!panel) {
    return;
  }

  const fields = panel.fieldsByTab[tab];
  const targetField = fields?.find((field) => field.key === fieldKey);

  if (!targetField) {
    return;
  }

  targetField.value = value;

  if (fieldKey === "nodeName") {
    nodes.value = nodes.value.map((node) =>
      node.id === selectedNodeId.value
        ? {
            ...node,
            data: {
              ...node.data,
              subtitle: value,
            },
          }
        : node,
    ) as WorkflowFlowNode[];
    syncSelectedNodeData();
  }
};

const handleAddSwitchBranch = () => {
  if (
    !isEditMode.value ||
    !isSelectedSwitchNode.value ||
    !selectedPanel.value
  ) {
    return;
  }

  pushHistorySnapshot();

  const branches = getSwitchBranches(selectedPanel.value);
  const nextBranch = {
    id: getNextSwitchBranchHandleId(selectedPanel.value),
    label: getNextSwitchBranchLabel(selectedPanel.value),
  };

  setSwitchBranches(selectedPanel.value, [...branches, nextBranch]);
  syncBranchHandleNodes(selectedNodeId.value);
  toast.success(`已新增分支：${nextBranch.label}`);
};

const handleSwitchBranchLabelUpdate = (branchId: string, value: string) => {
  if (
    !isEditMode.value ||
    !isSelectedSwitchNode.value ||
    !selectedPanel.value
  ) {
    return;
  }

  setSwitchBranches(
    selectedPanel.value,
    getSwitchBranches(selectedPanel.value).map((branch) =>
      branch.id === branchId
        ? {
            ...branch,
            label: value,
          }
        : branch,
    ),
  );
  syncBranchHandleNodes(selectedNodeId.value);
};

const handleSwitchFallbackUpdate = (branchId: string) => {
  if (
    !isEditMode.value ||
    !isSelectedSwitchNode.value ||
    !selectedPanel.value
  ) {
    return;
  }

  setSwitchFallbackHandle(selectedPanel.value, branchId);
  syncBranchHandleNodes(selectedNodeId.value);
};

const handleRemoveSwitchBranch = (branchId: string) => {
  if (
    !isEditMode.value ||
    !isSelectedSwitchNode.value ||
    !selectedPanel.value
  ) {
    return;
  }

  const branches = getSwitchBranches(selectedPanel.value);

  if (branches.length <= 2) {
    toast.info("Switch 节点至少需要保留两个分支");
    return;
  }

  pushHistorySnapshot();

  const nextBranches = branches.filter((branch) => branch.id !== branchId);
  const previousFallbackHandle = getSwitchFallbackHandle(selectedPanel.value);

  setSwitchBranches(selectedPanel.value, nextBranches);

  if (previousFallbackHandle === branchId) {
    setSwitchFallbackHandle(
      selectedPanel.value,
      nextBranches[nextBranches.length - 1]?.id ?? "",
    );
  }

  edges.value = edges.value.filter(
    (edge) =>
      edge.source !== selectedNodeId.value || edge.sourceHandle !== branchId,
  );
  syncBranchHandleNodes(selectedNodeId.value);
  toast.success("已移除分支并清理对应连线");
};

const handleRunDraftUpdate = <K extends keyof WorkflowRunDraft>(
  key: K,
  value: WorkflowRunDraft[K],
) => {
  runDraft.value = {
    ...runDraft.value,
    [key]: value,
  };
};

const parseJsonRecord = (rawValue: string, fieldLabel: string) => {
  const trimmed = rawValue.trim();

  if (!trimmed) {
    return {};
  }

  let parsed: unknown;

  try {
    parsed = JSON.parse(trimmed);
  } catch {
    throw new Error(`${fieldLabel} 需要是合法的 JSON 对象`);
  }

  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error(`${fieldLabel} 需要是 JSON 对象`);
  }

  return parsed as Record<string, unknown>;
};

const normalizeRunEnvRecord = (env: Record<string, unknown>) => {
  const nextEnv = { ...env };

  if (
    typeof nextEnv.siteCode === "string" &&
    typeof nextEnv.warehouseId !== "string"
  ) {
    nextEnv.warehouseId = nextEnv.siteCode;
  }

  if (typeof nextEnv.tenantId !== "string" || !nextEnv.tenantId.trim()) {
    nextEnv.tenantId = "tenant-a";
  }

  return nextEnv;
};

const buildRunExecutionRequest = () => {
  const body = parseJsonRecord(runDraft.value.body, "Trigger Body");
  const env = normalizeRunEnvRecord(
    parseJsonRecord(runDraft.value.env, "运行环境变量"),
  );

  if (runDraft.value.triggerMode === "webhook") {
    return {
      env,
      trigger: {
        body,
        headers: parseJsonRecord(runDraft.value.headers, "Webhook Headers"),
      },
    };
  }

  return {
    env,
    trigger: {
      body,
    },
  };
};

const formatJsonPreview = (value: unknown) =>
  JSON.stringify(value ?? {}, null, 2);

const clearRunSummaryPolling = () => {
  if (runSummaryPollTimer !== null) {
    window.clearTimeout(runSummaryPollTimer);
    runSummaryPollTimer = null;
  }
};

const selectRunFocusedNode = (summary: WorkflowRunSummary) => {
  const candidateNodeId =
    summary.currentNodeId ??
    summary.timeline[summary.timeline.length - 1]?.nodeId;

  if (
    candidateNodeId &&
    nodes.value.some((node) => node.id === candidateNodeId)
  ) {
    setSelectedNode(candidateNodeId);
  }
};

const refreshRunSummary = async () => {
  if (!activeRunId.value) {
    return;
  }

  try {
    const summary = await fetchWorkflowRunSummary(activeRunId.value);

    activeRunSummary.value = summary;
    runErrorMessage.value = "";
    setNodeExecutionStatuses(summary);
    selectRunFocusedNode(summary);

    if (shouldPollWorkflowRunSummary(summary.status)) {
      clearRunSummaryPolling();
      runSummaryPollTimer = window.setTimeout(() => {
        void refreshRunSummary();
      }, 1200);
      return;
    }

    isTerminatingWorkflow.value = false;
    clearRunSummaryPolling();
  } catch (error) {
    isTerminatingWorkflow.value = false;
    clearRunSummaryPolling();
    runErrorMessage.value =
      error instanceof Error ? error.message : "获取运行状态失败";
  }
};

const handleRefreshRunSummary = async () => {
  if (!activeRunId.value) {
    toast.info("当前还没有运行记录");
    return;
  }

  await refreshRunSummary();
};

const handleTerminateRun = async () => {
  if (
    !activeRunId.value ||
    !canTerminateActiveRun.value ||
    isTerminatingWorkflow.value
  ) {
    return;
  }

  isTerminatingWorkflow.value = true;
  runErrorMessage.value = "";

  try {
    const summary = await terminateWorkflowRun(activeRunId.value);
    activeRunSummary.value = summary;
    setNodeExecutionStatuses(summary);

    if (summary.status === "terminated") {
      isTerminatingWorkflow.value = false;
      clearRunSummaryPolling();
      toast.success(`运行已终止：${summary.runId}`);
      return;
    }

    toast.success("已发送终止请求");
    await refreshRunSummary();
  } catch (error) {
    isTerminatingWorkflow.value = false;
    runErrorMessage.value =
      error instanceof Error ? error.message : "终止工作流运行失败";
    toast.error(runErrorMessage.value);
  }
};

const handleRunWorkflow = async () => {
  if (isRunningWorkflow.value) {
    return;
  }

  isRunningWorkflow.value = true;
  runErrorMessage.value = "";
  pageMode.value = "run";

  try {
    const editorDocument = createPersistedWorkflowDocument(
      nodes.value,
      edges.value,
      panelByNodeId.value,
      {
        activeTab: activeTab.value,
        pageMode: "run",
        runDraft: runDraft.value,
        selectedNodeId: selectedNodeId.value,
        status: workflowMeta.status,
        version: workflowMeta.version,
        workflowId: workflowMeta.id,
        workflowName: workflowMeta.name,
      },
    );
    const registration = await syncWorkflowToRunner(
      nodes.value,
      edges.value,
      panelByNodeId.value,
      {
        editorDocument,
        persistedWorkflowId: persistedWorkflowId.value,
        workflowId: workflowMeta.id,
        workflowName: workflowMeta.name,
        workflowVersion: workflowMeta.version,
        workflowStatus: workflowMeta.status,
      },
    );
    const execution = await executeWorkflowRun(
      registration.workflowId,
      buildRunExecutionRequest(),
    );

    workflowMeta.id = registration.workflowId;
    activeRunWorkflowId.value = registration.workflowId;
    activeRunId.value = execution.runId;
    activeRunSummary.value = {
      currentNodeId: undefined,
      runId: execution.runId,
      state: {},
      status: "running",
      timeline: [],
      workflowKey: registration.workflowKey,
      workflowVersion: registration.workflowVersion,
    };
    setNodeExecutionStatuses(activeRunSummary.value);
    void router.replace({
      name: "workflow-editor",
      params: {
        id: registration.workflowId,
      },
      query: {
        runId: execution.runId,
      },
    });

    toast.success(`已启动运行：${execution.runId}`);
    await refreshRunSummary();
  } catch (error) {
    runErrorMessage.value =
      error instanceof Error ? error.message : "启动工作流运行失败";
    toast.error(runErrorMessage.value);
  } finally {
    isRunningWorkflow.value = false;
  }
};

const handleExportJson = () => {
  try {
    const exportDocument = createWorkflowExportDocument(
      nodes.value,
      edges.value,
      panelByNodeId.value,
      {
        selectedNodeId: selectedNodeId.value,
        status: workflowMeta.status,
        version: workflowMeta.version,
        workflowId: workflowMeta.id,
        workflowName: workflowMeta.name,
      },
    );
    const blob = new Blob([`${JSON.stringify(exportDocument, null, 2)}\n`], {
      type: "application/json;charset=utf-8",
    });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");

    link.href = url;
    link.download = `${workflowMeta.name}.${workflowMeta.version}.json`;
    document.body.appendChild(link);
    link.click();
    link.remove();
    URL.revokeObjectURL(url);

    toast.success("工作流 JSON 已导出");
  } catch (error) {
    toast.error(
      error instanceof Error ? error.message : "导出工作流 JSON 失败",
    );
  }
};

const handlePublish = async () => {
  if (isPublishing.value) {
    return;
  }

  isPublishing.value = true;

  try {
    const persistedDocument = createPersistedWorkflowDocument(
      nodes.value,
      edges.value,
      panelByNodeId.value,
      {
        activeTab: activeTab.value,
        pageMode: pageMode.value,
        runDraft: runDraft.value,
        selectedNodeId: selectedNodeId.value,
        status: "published",
        version: workflowMeta.version,
        workflowId: workflowMeta.id,
        workflowName: workflowMeta.name,
      },
    );
    const registration = await publishWorkflowToRunner(
      nodes.value,
      edges.value,
      panelByNodeId.value,
      {
        editorDocument: persistedDocument,
        persistedWorkflowId: persistedWorkflowId.value,
        workflowId: workflowMeta.id,
        workflowName: workflowMeta.name,
        workflowVersion: workflowMeta.version,
        workflowStatus: workflowMeta.status,
      },
    );
    const publishedWorkflowId =
      registration.workflowId?.trim() || workflowMeta.id;

    workflowMeta.id = publishedWorkflowId;
    workflowMeta.status = "published";
    await router.replace({
      name: "workflow-editor",
      params: {
        id: publishedWorkflowId,
      },
    });
    toast.success(`已发布到 Runner：${publishedWorkflowId}`);
  } catch (error) {
    toast.error(error instanceof Error ? error.message : "发布到 Runner 失败");
    console.error(error);
  } finally {
    isPublishing.value = false;
  }
};

const isEditableTarget = (target: EventTarget | null) => {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  const tagName = target.tagName.toLowerCase();

  if (target.isContentEditable) {
    return true;
  }

  return tagName === "input" || tagName === "textarea" || tagName === "select";
};

const handleWindowKeydown = (event: KeyboardEvent) => {
  if (isEditableTarget(event.target)) {
    return;
  }

  if (!isEditMode.value) {
    return;
  }

  if (
    (event.metaKey || event.ctrlKey) &&
    !event.shiftKey &&
    event.key.toLowerCase() === "z"
  ) {
    event.preventDefault();
    undoLastChange();
    return;
  }

  if (event.key === "Delete" || event.key === "Backspace") {
    event.preventDefault();
    deleteSelectedNode();
  }
};

onMounted(() => {
  window.addEventListener("keydown", handleWindowKeydown);
});

onPaneReady(() => {
  isCanvasPaneReady.value = true;

  if (!shouldResetCanvasViewport.value) {
    return;
  }

  shouldResetCanvasViewport.value = false;
  void resetCanvasViewport();
});

onBeforeUnmount(() => {
  closeAssistantSessionSocket();
  clearRunSummaryPolling();
  window.removeEventListener("keydown", handleWindowKeydown);
});

setSelectedNode(selectedNodeId.value);
</script>

<style scoped>
.workflow-canvas :deep(.vue-flow__pane) {
  background-color: transparent;
}

.workflow-canvas :deep(.vue-flow__edge-path) {
  stroke: #cbd5e1;
  stroke-width: 2;
}

.scrollbar-hide::-webkit-scrollbar {
  display: none;
}
.scrollbar-hide {
  -ms-overflow-style: none;
  scrollbar-width: none;
}
</style>
