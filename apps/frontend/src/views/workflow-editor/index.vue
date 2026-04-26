<template>
  <section
    class="workflow-editor-shell relative h-screen w-full overflow-hidden"
  >
    <!-- Main Canvas takes full absolute space -->
    <main
      class="workflow-canvas absolute inset-0 z-0 h-full w-full transition-opacity duration-150"
      :class="canvasVisibilityClass"
      @dragenter.prevent="handleCanvasDragEnter"
      @dragover.prevent="handleCanvasDragOver"
      @drop.prevent="handleCanvasDrop"
    >
      <VueFlow
        id="workflow-editor-flow"
        v-model:nodes="nodes"
        v-model:edges="edges"
        :delete-key-code="isAiMode ? null : undefined"
        :disable-keyboard-a11y="isAiMode"
        :edges-focusable="!isAiMode"
        :nodes-focusable="!isAiMode"
        :selection-key-code="isAiMode ? false : undefined"
        class="h-full w-full"
        @connect="handleConnect"
        @edges-change="handleEdgesChange"
        @node-click="handleNodeClick"
        @nodes-change="handleNodesChange"
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
          class="absolute inset-4 rounded-[28px] border-2 border-dashed border-[var(--app-accent)]/45 bg-[var(--app-accent)]/6 shadow-[inset_0_0_0_1px_rgba(46,198,214,0.12)]"
        >
          <div class="absolute inset-x-0 top-6 flex justify-center">
            <div
              class="rounded-full bg-white/94 px-4 py-2 text-xs font-semibold tracking-[0.03em] text-[var(--app-accent-text)] shadow-sm"
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
        <ElButton text
          class="h-8 w-8 rounded-full text-[var(--app-muted)] hover:bg-[var(--app-primary-soft)]"
          @click="handleBackToList"
        >
          <ChevronLeft class="h-5 w-5" />
        </ElButton>
        <span class="text-[16px] font-semibold tracking-tight text-[var(--text)]">{{
          workflowTitle
        }}</span>
        <span
          class="rounded-full bg-[var(--app-accent-soft)]/75 px-2 py-0.5 text-[11px] font-semibold text-[var(--app-accent-text)]"
          >{{ workflowStatusLabel }}</span
        >
        <ElButton
          v-if="persistedWorkflowId"
 text
          class="h-8 gap-1.5 rounded-full px-3 text-sm font-medium text-[var(--app-muted)] hover:bg-[var(--app-primary-soft)]"
          @click="handleOpenWorkflowRuns"
        >
          <LoaderCircle class="h-3.5 w-3.5" />
          查看运行
          <span
            class="inline-flex min-w-[1.35rem] items-center justify-center rounded-full bg-[var(--app-primary)] px-1.5 py-0.5 text-[10px] font-semibold leading-none text-white"
          >
            {{ workflowRunCount }}
          </span>
        </ElButton>
      </div>
      <div
        class="pointer-events-auto absolute left-1/2 flex h-9 -translate-x-1/2 items-center rounded-full bg-white/92 p-1 shadow-sm ring-1 ring-[var(--panel-border)]"
      >
        <button
          class="flex h-7 w-11 items-center justify-center rounded-full transition-colors cursor-pointer"
          :class="
            isEditMode
              ? 'bg-[var(--app-accent-soft)] text-[var(--app-accent-text)]'
              : 'text-[#7a7f86] hover:bg-white hover:text-[var(--text)]'
          "
          @click="handlePageModeChange('edit')"
        >
          <Pencil class="h-3.5 w-3.5" />
        </button>
        <button
          class="flex h-7 w-11 items-center justify-center rounded-full transition-colors cursor-pointer"
          :class="
            isRunMode
              ? 'bg-[var(--app-accent-soft)] text-[var(--app-accent-text)]'
              : 'text-[#7a7f86] hover:bg-white hover:text-[var(--text)]'
          "
          @click="handlePageModeChange('run')"
        >
          <Play class="h-3.5 w-3.5" />
        </button>
        <button
          class="flex h-7 w-11 items-center justify-center rounded-full transition-colors cursor-pointer"
          :class="
            isAiMode
              ? 'bg-[var(--app-accent-soft)] text-[var(--app-accent-text)]'
              : 'text-[#7a7f86] hover:bg-white hover:text-[var(--text)]'
          "
          @click="handlePageModeChange('ai')"
        >
          <Bot class="h-3.5 w-3.5" />
        </button>
      </div>
      <div class="flex items-center gap-1.5 pointer-events-auto">
        <WorkflowHeaderActionButtons appearance="compact" />
        <ElButton text
          class="h-8 gap-1.5 rounded-full px-3 text-sm font-medium text-[var(--app-muted)] hover:bg-[var(--app-primary-soft)]"
        >
          <Compass class="h-4 w-4" />
          Evaluate
        </ElButton>
        <ElButton text
          class="h-8 gap-1.5 rounded-full px-3 text-sm font-medium text-[var(--app-muted)] hover:bg-[var(--app-primary-soft)]"
          @click="handleExportJson"
        >
          <Code class="h-4 w-4" />
          JSON
        </ElButton>
        <ElButton
          class="ml-1 h-8 rounded-full bg-[var(--app-primary)] px-4 text-[13px] font-medium text-white shadow-sm hover:bg-[#354a56] disabled:cursor-not-allowed disabled:opacity-60"
          :disabled="isPublishing"
          @click="handlePublish"
        >
          {{ publishButtonLabel }}
        </ElButton>
      </div>
    </header>
    <WorkflowRunListDialog
      :open="isWorkflowRunListOpen"
      :workflow-id="persistedWorkflowId ?? ''"
      :workflow-name="workflowTitle"
      @update:open="handleWorkflowRunListOpenChange"
      @select-run="handleOpenWorkflowRunFromList"
    />
    <WorkflowAiChatPanel
      v-if="isAiMode"
      :runner-base-url="assistantRunnerBaseUrl"
      :runner-connection-label="assistantConnectionLabel"
      :runner-connection-state="assistantConnectionState"
      :runner-preview-updated-at="assistantSession?.updatedAt"
      :session-error="assistantSessionError"
      :session-id="assistantSessionId"
      :visibility-class="rightAsideVisibilityClass"
      :workflow-id="persistedWorkflowId"
    />
    <!-- Floating Left Panel -->
    <aside
      v-if="isEditMode"
      ref="leftCanvasAsideRef"
      class="pointer-events-auto absolute left-6 top-24 bottom-auto z-10 flex max-h-[calc(100vh-7.5rem)] w-60 flex-col overflow-hidden rounded-[20px] bg-white/92 backdrop-blur shadow-sm ring-1 ring-[var(--panel-border)]/80"
    >
      <div class="min-h-0 flex-1 overflow-y-auto py-3 px-2">
        <div
          v-for="category in filteredCategories"
          :key="category.id"
          class="mb-4 last:mb-0"
        >
          <div class="mb-1.5 px-3 text-[11px] font-medium text-[#7a7f86]">
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
                isPaletteItemSelected(item)
                  ? 'bg-[var(--app-accent-soft)]/70'
                  : 'hover:bg-[var(--panel-soft)]'
              "
              @click="focusPaletteItem(item)"
              @dragstart="handlePaletteDragStart($event, item.id)"
              @dragend="handlePaletteDragEnd"
            >
              <div
                class="flex h-6.5 w-6.5 items-center justify-center rounded-lg bg-[var(--app-primary-soft)]"
                :style="{
                  color: item.accent,
                  backgroundColor: `${item.accent}15`,
                }"
              >
                <WorkflowIcon
                  :icon="item.icon"
                  :alt="item.label"
                  class="h-3.5 w-3.5"
                />
              </div>
              <div class="min-w-0 flex-1">
                <p class="truncate text-[13px] font-medium text-[#354a56]">
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
      ref="leftCanvasAsideRef"
      class="pointer-events-auto absolute left-6 top-24 bottom-auto z-10 flex max-h-[calc(100vh-7.5rem)] w-[320px] flex-col overflow-hidden rounded-[20px] bg-white/92 backdrop-blur shadow-sm ring-1 ring-[var(--panel-border)]/80"
    >
      <div
        class="flex h-21 shrink-0 items-center justify-between border-b border-[var(--panel-border)]/55 px-4"
      >
        <div>
          <p class="text-[14px] font-semibold text-[var(--text)]">运行配置</p>
          <p class="mt-1 text-[11px] leading-5 text-[#7a7f86]">
            同步当前画布到 Runner 后立即执行，适合联调节点映射和分支结果。
          </p>
        </div>
        <span
          class="truncate rounded-full bg-[var(--app-primary-soft)] px-2.5 py-1 text-[11px] font-semibold text-[var(--app-muted)]"
          >{{ runnerTriggerSummaryLabel }}</span
        >
      </div>
      <div class="min-h-0 flex-1 space-y-4 overflow-y-auto px-4 py-4">
        <div
          class="rounded-[18px] border border-[var(--panel-border)]/80 bg-[var(--panel-soft)]/90 p-3"
        >
          <p class="text-xs font-semibold tracking-wide text-[var(--app-muted)]">
            触发载荷
          </p>
          <div
            class="mt-3 flex rounded-full bg-white p-1 ring-1 ring-[var(--panel-border)]"
          >
            <button
              type="button"
              class="flex-1 rounded-full px-3 py-1.5 text-xs font-medium transition-colors"
              :class="
                runDraft.triggerMode === 'manual'
                  ? 'bg-[var(--app-primary)] text-white'
                  : 'text-[var(--app-muted)] hover:text-[var(--text)]'
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
                  ? 'bg-[var(--app-primary)] text-white'
                  : 'text-[var(--app-muted)] hover:text-[var(--text)]'
              "
              @click="handleRunDraftUpdate('triggerMode', 'webhook')"
            >
              Webhook
            </button>
          </div>
          <p class="mt-3 text-[11px] leading-5 text-[var(--app-muted)]">
            `Manual` 只发送 `body`，`Webhook` 会附带 `headers +
            body`，方便模拟真实入口。
          </p>
        </div>
        <div class="space-y-1.5">
          <label
            class="block text-xs font-semibold tracking-wide text-[var(--app-muted)]"
            >Trigger Body</label
          >
          <textarea
            :value="runDraft.body"
            class="min-h-37 w-full rounded-2xl border border-[var(--panel-border)] bg-white px-3 py-3 font-mono text-[12px] leading-6 text-[var(--text)] outline-none transition focus:border-[var(--app-accent-border)] focus:ring-2 focus:ring-[var(--app-accent-soft)]"
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
            class="block text-xs font-semibold tracking-wide text-[var(--app-muted)]"
            >Webhook Headers</label
          >
          <textarea
            :value="runDraft.headers"
            class="min-h-28 w-full rounded-2xl border border-[var(--panel-border)] bg-white px-3 py-3 font-mono text-[12px] leading-6 text-[var(--text)] outline-none transition focus:border-[var(--app-accent-border)] focus:ring-2 focus:ring-[var(--app-accent-soft)]"
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
            class="block text-xs font-semibold tracking-wide text-[var(--app-muted)]"
            >Run Env</label
          >
          <textarea
            :value="runDraft.env"
            class="min-h-28 w-full rounded-2xl border border-[var(--panel-border)] bg-white px-3 py-3 font-mono text-[12px] leading-6 text-[var(--text)] outline-none transition focus:border-[var(--app-accent-border)] focus:ring-2 focus:ring-[var(--app-accent-soft)]"
            @input="
              handleRunDraftUpdate(
                'env',
                ($event.target as HTMLTextAreaElement).value,
              )
            "
          />
        </div>
        <div class="rounded-[18px] border border-[var(--panel-border)]/80 bg-white p-3">
          <div class="flex items-center justify-between text-xs text-[var(--app-muted)]">
            <span class="font-semibold">执行预览</span>
            <span>{{ runnerWorkflowPreview.nodes.length }} nodes</span>
          </div>
          <div class="mt-3 grid grid-cols-2 gap-2 text-xs">
            <div class="rounded-xl bg-[var(--panel-soft)] px-3 py-2">
              <p class="text-[#7a7f86]">Trigger</p>
              <p class="mt-1 font-semibold text-[#354a56]">
                {{ runnerWorkflowPreview.trigger.type }}
              </p>
            </div>
            <div class="rounded-xl bg-[var(--panel-soft)] px-3 py-2">
              <p class="text-[#7a7f86]">Transitions</p>
              <p class="mt-1 font-semibold text-[#354a56]">
                {{ runnerWorkflowPreview.transitions.length }}
              </p>
            </div>
          </div>
        </div>
      </div>
      <div class="shrink-0 border-t border-[var(--panel-border)]/55 px-4 py-4">
        <ElButton
          class="h-10 w-full rounded-full bg-[var(--app-primary)] text-sm font-medium text-white hover:bg-[#354a56] disabled:cursor-not-allowed disabled:opacity-60"
          :disabled="isRunningWorkflow"
          @click="handleRunWorkflow"
        >
          <LoaderCircle v-if="isRunningWorkflow" class="h-4 w-4 animate-spin" />
          <Play v-else class="h-4 w-4" />
          {{ runActionLabel }}
        </ElButton>
        <p class="mt-2 text-[11px] leading-5 text-[#7a7f86]">
          运行不会自动发布正式版本，但会把当前画布同步到 Runner
          进行一次最新执行。
        </p>
      </div>
    </aside>
    <!-- Floating Right Properties Panel -->
    <aside
      v-if="isEditMode && selectedNodeId"
      class="pointer-events-auto absolute right-6 top-24 bottom-auto z-10 flex max-h-[calc(100vh-7.5rem)] w-[320px] flex-col overflow-hidden rounded-[20px] bg-white/92 backdrop-blur shadow-sm ring-1 ring-[var(--panel-border)]/80"
      :class="rightAsideVisibilityClass"
    >
      <div
        class="flex h-17 shrink-0 items-center gap-3 border-b border-[var(--panel-border)]/55 px-4"
      >
        <div
          class="flex h-9 w-9 shrink-0 items-center justify-center rounded-[10px] text-white shadow-sm"
          :style="{ backgroundColor: selectedNodeData.accent }"
        >
          <WorkflowIcon
            :icon="selectedNodeData.icon"
            :alt="selectedNodeData.title"
            class="h-4.5 w-4.5"
          />
        </div>
        <div class="min-w-0 flex-1 px-1">
          <p class="truncate text-[14px] font-semibold text-[var(--text)]">
            {{ selectedNodeData.subtitle ?? selectedNodeData.title }}
          </p>
          <p class="truncate text-[11px] font-medium text-[#7a7f86]">
            {{ selectedNodeData.title }}
          </p>
        </div>
        <button
          type="button"
          class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-[#7a7f86] transition-colors hover:bg-[var(--app-primary-soft)] hover:text-[var(--app-muted)]"
          @click="clearSelectedNode"
        >
          <MoreHorizontal class="h-4 w-4" />
        </button>
      </div>
      <ElTabs
        class="flex min-h-0 flex-1 flex-col"
        :model-value="activeTab"
        @update:model-value="handleTabChange"
      >
        <ElTabPane
          v-for="tab in visibleTabs"
          :key="tab"
          :name="tab"
        >
          <template #label>
            <span :data-tab-visible="`${tab}`" class="px-2 text-xs font-medium">
              {{ WORKFLOW_TAB_LABELS[tab] }}
            </span>
          </template>
          <div class="min-h-0 flex-1 overflow-y-auto px-4 py-4">
            <div
              v-if="
                getFieldsForTab(tab).length ||
                (isSelectedSwitchNode && tab === 'mapping')
              "
              class="space-y-4"
            >
              <div
                v-if="isSelectedSwitchNode && tab === 'mapping'"
                class="space-y-3 rounded-2xl border border-[var(--panel-border)] bg-[var(--panel-soft)]/90 p-3"
              >
                <div class="flex items-center justify-between gap-3">
                  <div>
                    <p
                      class="text-xs font-semibold tracking-wide text-[var(--app-muted)]"
                    >
                      Switch 分支
                    </p>
                    <p class="mt-1 text-[11px] leading-5 text-[#7a7f86]">
                      每个分支对应一个独立出口，默认分支会在没有匹配时生效。
                    </p>
                  </div>
                  <button
                    type="button"
                    class="inline-flex h-8 items-center justify-center gap-1 truncate rounded-full border border-[var(--panel-border)] bg-white px-3 text-xs font-semibold text-[#354a56] transition-colors hover:border-[var(--app-accent-border)] hover:bg-[var(--app-accent-soft)]/40"
                    @click="handleAddSwitchBranch"
                  >
                    <Plus class="h-3.5 w-3.5" />
                    添加分支
                  </button>
                </div>
                <div
                  v-for="branch in selectedSwitchBranches"
                  :key="branch.id"
                  class="rounded-[14px] border border-[var(--panel-border)] bg-white p-3"
                >
                  <div class="flex items-start gap-2">
                    <div class="min-w-0 flex-1 space-y-1.5">
                      <label
                        class="block text-[11px] font-semibold tracking-wide text-[var(--app-muted)]"
                      >
                        分支标签
                      </label>
                      <ElInput
                        :model-value="branch.label"
                        class="h-9 rounded-lg border-[var(--panel-border)] bg-white px-3 text-sm shadow-none focus-visible:border-[var(--app-accent-border)] focus-visible:ring-2 focus-visible:ring-[var(--app-accent-soft)]"
                        @update:model-value="
                          handleSwitchBranchLabelUpdate(
                            branch.id,
                            String($event),
                          )
                        "
                      />
                    </div>
                    <button
                      type="button"
                      class="mt-6 inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-full border transition-colors disabled:cursor-not-allowed disabled:opacity-50"
                      :class="
                        selectedSwitchFallbackHandle === branch.id
                          ? 'border-[var(--app-primary)] bg-[var(--app-primary)] text-white'
                          : 'border-[var(--panel-border)] bg-white text-[var(--app-muted)] hover:border-[var(--app-accent-border)] hover:text-[var(--text)]'
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
                      class="mt-6 inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-full border border-[var(--panel-border)] bg-white text-[#7a7f86] transition-colors hover:border-[var(--danger-border)] hover:text-[var(--danger-text)] disabled:cursor-not-allowed disabled:opacity-50"
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
                :data-filed-tab="`${tab}-${field.key}`"
                class="space-y-1.5"
              >
                <label
                  class="flex items-center justify-between gap-3 text-xs font-semibold tracking-wide text-[var(--app-muted)]"
                >
                  <span>{{ field.label }}</span>
                  <a
                    v-if="getSubWorkflowLinkHref(field)"
                    :href="getSubWorkflowLinkHref(field)"
                    target="_blank"
                    rel="noreferrer"
                    class="shrink-0 text-xs font-medium tracking-normal text-[var(--info)] transition-colors hover:text-[var(--info-text)] hover:underline"
                  >
                    打开子工作流
                  </a>
                </label>
                <ElInput
                  v-if="field.type === 'input'"
                  :model-value="field.value"
                  class="h-9 rounded-lg border-[var(--panel-border)] bg-white px-3 text-sm shadow-none focus-visible:border-[var(--app-accent-border)] focus-visible:ring-2 focus-visible:ring-[var(--app-accent-soft)]"
                  @update:model-value="
                    handleFieldUpdate(tab, field.key, String($event))
                  "
                />
                <div v-else-if="field.type === 'select'" class="relative">
                  <select
                    :value="field.value"
                    class="h-9 w-full appearance-none rounded-lg border border-[var(--panel-border)] bg-white px-3 pr-9 text-sm text-[var(--text)] shadow-none outline-none transition focus:border-[var(--app-accent-border)] focus:ring-2 focus:ring-[var(--app-accent-soft)]"
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
                    class="pointer-events-none absolute right-3 top-1/2 h-4 w-4 -translate-y-1/2 text-[#7a7f86]"
                  />
                </div>
                <textarea
                  v-else-if="field.type === 'textarea'"
                  :value="field.value"
                  class="min-h-20 w-full rounded-lg border border-[var(--panel-border)] bg-white px-3 py-2 text-sm text-[var(--text)] shadow-none outline-none transition focus:border-[var(--app-accent-border)] focus:ring-2 focus:ring-[var(--app-accent-soft)]"
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
                      ? 'border-[var(--panel-border)]/40 bg-[var(--panel-soft)] text-[var(--app-muted)]'
                      : 'border-[var(--panel-border)] bg-white text-[var(--text)]'
                  "
                >
                  <span>{{ field.value }}</span>
                </div>
              </div>
            </div>
            <div
              v-else
              class="flex min-h-40 items-center justify-center rounded-xl border border-dashed border-[var(--panel-border)] bg-[var(--panel-soft)]/70 px-6 text-center text-xs leading-5 text-[#7a7f86]"
            >
              {{ WORKFLOW_EMPTY_TAB_TEXT[tab] }}
            </div>
          </div>
        </ElTabPane>
      </ElTabs>
    </aside>
    <aside
      v-else-if="isRunMode"
      class="pointer-events-auto absolute right-6 top-24 bottom-auto z-10 flex max-h-[calc(100vh-7.5rem)] w-90 flex-col overflow-hidden rounded-[20px] bg-white/92 backdrop-blur shadow-sm ring-1 ring-[var(--panel-border)]/80"
      :class="rightAsideVisibilityClass"
    >
      <div
        class="flex h-18 shrink-0 items-center gap-3 border-b border-[var(--panel-border)]/55 px-4"
      >
        <div
          class="flex h-9.5 w-9.5 items-center justify-center rounded-[12px] bg-[var(--app-primary)] text-white shadow-sm"
        >
          <Webhook v-if="runDraft.triggerMode === 'webhook'" class="h-4 w-4" />
          <Play v-else class="h-4 w-4" />
        </div>
        <div class="min-w-0 flex-1">
          <p class="truncate text-[14px] font-semibold text-[var(--text)]">
            运行结果
          </p>
          <p class="truncate text-[11px] text-[#7a7f86]">
            {{
              activeRunId
                ? `Run ${activeRunId}`
                : "执行后会在这里看到状态和 timeline"
            }}
          </p>
        </div>
        <button
          type="button"
          class="flex h-8 w-8 items-center justify-center rounded-full text-[#7a7f86] transition-colors hover:bg-[var(--app-primary-soft)] hover:text-[var(--app-muted)] disabled:cursor-not-allowed disabled:opacity-50"
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
        <div class="rounded-[18px] border border-[var(--panel-border)]/80 bg-white p-4">
          <div class="flex items-start justify-between gap-3">
            <div>
              <p class="text-xs font-semibold tracking-wide text-[var(--app-muted)]">
                运行状态
              </p>
              <p
                class="mt-2 text-2xl font-semibold tracking-tight text-[var(--text)]"
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
            class="mt-4 inline-flex h-9 items-center justify-center gap-2 rounded-full border border-[var(--danger-border)] px-3 text-sm font-medium text-[var(--danger-text)] transition-colors hover:bg-[var(--danger-soft)] disabled:cursor-not-allowed disabled:border-[var(--panel-border)] disabled:text-[#7a7f86] disabled:hover:bg-transparent"
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
          <div class="mt-4 space-y-2 text-xs text-[var(--app-muted)]">
            <div class="flex items-center justify-between gap-3">
              <span>Workflow ID</span>
              <span class="truncate font-medium text-[#354a56]">{{
                activeRunWorkflowId || workflowMeta.id
              }}</span>
            </div>
            <div class="flex items-center justify-between gap-3">
              <span>Current Node</span>
              <span class="truncate font-medium text-[#354a56]">{{
                activeRunSummary?.currentNodeId ?? "--"
              }}</span>
            </div>
            <div class="flex items-center justify-between gap-3">
              <span>Timeline Steps</span>
              <span class="font-medium text-[#354a56]">{{
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
        <WorkflowRunTimelineDetail
          :summary="activeRunSummary"
          :node-name-map="workflowNodeNameMap"
          :workflow-key="activeRunSummary?.workflowKey ?? runnerWorkflowPreview.meta.key"
        />
        <div class="rounded-[18px] border border-[var(--panel-border)]/80 bg-white p-4">
          <p class="text-xs font-semibold tracking-wide text-[var(--app-muted)]">
            State Snapshot
          </p>
          <pre
            class="mt-3 max-h-45 overflow-auto rounded-[14px] bg-[var(--app-primary)] px-3 py-3 font-mono text-[11px] leading-5 text-[#f5f7f7]"
            >{{ runStatePreview }}</pre
          >
        </div>
        <div class="rounded-[18px] border border-[var(--panel-border)]/80 bg-white p-4">
          <p class="text-xs font-semibold tracking-wide text-[var(--app-muted)]">
            Last Output
          </p>
          <pre
            class="mt-3 max-h-45 overflow-auto rounded-[14px] bg-[var(--app-primary)] px-3 py-3 font-mono text-[11px] leading-5 text-[#f5f7f7]"
            >{{ runOutputPreview }}</pre
          >
        </div>
      </div>
    </aside>
    <!-- Floating Bottom Control Toolbar -->
    <div
      v-if="isEditMode"
      class="pointer-events-auto absolute bottom-6 left-1/2 z-20 flex -translate-x-1/2 items-center gap-0.5 rounded-full bg-white/92 p-1 shadow-sm ring-1 ring-[var(--panel-border)]"
    >
      <button
        class="flex h-9 w-10 items-center justify-center rounded-full bg-[var(--app-accent-soft)] text-[var(--app-accent-text)] transition-colors"
      >
        <Hand class="h-4 w-4" />
      </button>
      <button
        class="flex h-9 w-10 items-center justify-center rounded-full text-[#7a7f86] transition-colors hover:bg-white hover:text-[var(--text)]"
      >
        <MousePointer2 class="h-4 w-4" />
      </button>
      <div class="mx-1 h-4 w-px bg-[var(--panel-border)]"></div>
      <button
        class="flex h-9 w-10 items-center justify-center rounded-full text-[#7a7f86] transition-colors hover:bg-white hover:text-[var(--text)]"
        @click="undoLastChange"
      >
        <Undo2 class="h-4 w-4" />
      </button>
      <button
        class="flex h-9 w-10 items-center justify-center rounded-full text-[#7a7f86] transition-colors hover:bg-white hover:text-[var(--text)]"
      >
        <Redo2 class="h-4 w-4" />
      </button>
    </div>
    <div
      v-if="isLoadingWorkflow"
      class="absolute inset-0 z-30 flex items-center justify-center bg-white/55 backdrop-blur-[2px]"
    >
      <div
        class="rounded-full border border-[var(--panel-border)] bg-white px-4 py-2 text-sm font-medium text-[var(--app-muted)] shadow-sm"
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
  shallowRef,
  triggerRef,
  watch,
} from "vue";
import {
  type Connection,
  type Edge,
  type EdgeChange,
  type NodeChange,
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
  Square,
  Trash2,
  Undo2,
  Webhook,
} from "lucide-vue-next";
import { type LocationQueryValue, useRoute, useRouter } from "vue-router";
import { toast } from "@/lib/element-toast";
import WorkflowAiChatPanel from "@/components/workflow/WorkflowAiChatPanel.vue";
import WorkflowBranchChipNode from "@/components/workflow/WorkflowBranchChipNode.vue";
import WorkflowCanvasNode from "@/components/workflow/WorkflowCanvasNode.vue";
import WorkflowIcon from "@/components/workflow/WorkflowIcon.vue";
import WorkflowHeaderActionButtons from "@/components/workflow/WorkflowHeaderActionButtons.vue";
import WorkflowRunTimelineDetail from "@/components/workflow/WorkflowRunTimelineDetail.vue";
import WorkflowRunListDialog from "@/components/workflow/WorkflowRunListDialog.vue";
import WorkflowTerminalNode from "@/components/workflow/WorkflowTerminalNode.vue";
import {
  fetchNodeDescriptors,
  fetchWorkflowList,
  fetchWorkflowRuns,
  fetchWorkflowDetail,
  type WorkflowDetail,
  type WorkflowSummary,
} from "@/features/workflow/api";
import { createWorkflowExportDocument } from "@/features/workflow/export";
import { createWorkflowEditorStateFromRunnerDefinition } from "@/features/workflow/import";
import {
  subscribeWorkflowEditSessionEvents,
  subscribeWorkflowRunEvents,
  subscribeWorkflowEvents,
} from "@/features/workflow/live";
import {
  clearWorkflowEditorSelection,
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
  createWorkflowEditSession,
  fetchWorkflowEditSession,
  type WorkflowEditSession,
} from "@/features/workflow/session";
import {
  WORKFLOW_EMPTY_TAB_TEXT,
  WORKFLOW_EDGE_STYLE,
  WORKFLOW_EDGE_TYPE,
  WORKFLOW_PALETTE_CATEGORIES,
  WORKFLOW_TAB_LABELS,
  createWorkflowPaletteCategories,
  createWorkflowPaletteItemMap,
  createSwitchBranchHandleId,
  createWorkflowNodeDraft,
  getWorkflowFieldSelectOptions,
  getSwitchBranches,
  getSwitchFallbackHandle,
  resolveWorkflowReferenceId,
  setSwitchBranches,
  setSwitchFallbackHandle,
  syncBranchHandlesForNode,
  type WorkflowExecutionStatus,
  type WorkflowField,
  type WorkflowFlowNode,
  type WorkflowIconKey,
  type WorkflowNodeData,
  type WorkflowNodeDescriptor,
  type WorkflowNodeKind,
  type WorkflowNodePanel,
  type WorkflowPaletteCategory,
  type WorkflowPaletteItem,
  type WorkflowTabId,
} from "@/features/workflow/model";
import type { EventSourceSubscription } from "@/lib/sse";
const DRAG_DATA_TYPE = "application/x-ses-workflow-node";
const HISTORY_LIMIT = 50;
const DEFAULT_WORKFLOW_ID = "sorting-main-flow";
const ASSISTANT_SESSION_POLL_INTERVAL_MS = 2000;
const RUN_SUMMARY_RESYNC_DELAY_MS = 1500;
const CANVAS_FIT_BASE_PADDING_PERCENT = 20;
const CANVAS_LEFT_ASIDE_GAP_PX = 24;
const CANVAS_LEFT_PADDING_MAX_RATIO = 0.45;
const route = useRoute();
const router = useRouter();
const initialEditorState = createInitialWorkflowEditorState();
const nodes = ref<WorkflowFlowNode[]>(initialEditorState.nodes);
const edges = ref<Edge[]>(initialEditorState.edges);
const panelByNodeId = shallowRef<Record<string, WorkflowNodePanel>>(
  initialEditorState.panelByNodeId,
);
const searchQuery = ref("");
const selectedNodeId = ref(initialEditorState.selectedNodeId);
const activeTab = ref<WorkflowTabId>(initialEditorState.activeTab);
const pageMode = ref<WorkflowPageMode>(initialEditorState.pageMode);
const runDraft = ref<WorkflowRunDraft>(initialEditorState.runDraft);
const leftCanvasAsideRef = ref<HTMLElement | null>(null);
const activeDragPaletteItemId = ref<string | null>(null);
const isCanvasDropTarget = ref(false);
const isPublishing = ref(false);
const isLoadingWorkflow = ref(false);
const isViewportResetting = ref(true);
const isRunningWorkflow = ref(false);
const isWorkflowRunListOpen = ref(false);
const isTerminatingWorkflow = ref(false);
const isCreatingAssistantSession = ref(false);
const assistantSession = ref<WorkflowEditSession | null>(null);
const assistantSessionError = ref("");
const assistantConnectionState = ref<
  "idle" | "connecting" | "live" | "reconnecting"
>("idle");
const historyStack = ref<WorkflowEditorSnapshot[]>([]);
const activeRunSummary = ref<WorkflowRunSummary | null>(null);
const activeRunId = ref("");
const activeRunWorkflowId = ref("");
const workflowRunCount = ref(0);
const workflowSummaries = ref<WorkflowSummary[]>([]);
const nodeDescriptors = ref<WorkflowNodeDescriptor[]>([]);
const runErrorMessage = ref("");
let runSummaryPollTimer: number | null = null;
let assistantSessionPollTimer: number | null = null;
let viewportResetTimer: number | null = null;
let assistantSessionPollInFlight = false;
let runSummaryRefreshInFlight = false;
let runSummaryRefreshQueued = false;
let runEventSubscription: EventSourceSubscription | null = null;
let assistantSessionEventSubscription: EventSourceSubscription | null = null;
let workflowRunCountSubscription: EventSourceSubscription | null = null;
let workflowRunCountRefreshInFlight = false;
let workflowRunCountRefreshQueued = false;
let pendingFlowRemovalSync: {
  edgeIds: Set<string>;
  nodeIds: Set<string>;
  nodeLabels: string[];
  selectedNodeRemoved: boolean;
} | null = null;
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
const paletteCategories = computed<WorkflowPaletteCategory[]>(() =>
  createWorkflowPaletteCategories(nodeDescriptors.value),
);
const WORKFLOW_FLOW_ID = "workflow-editor-flow";
const { fitView, onPaneReady, screenToFlowCoordinate } =
  useVueFlow(WORKFLOW_FLOW_ID);
const isCanvasPaneReady = ref(false);
const shouldResetCanvasViewport = ref(false);
const canvasTools = [
  { id: "select", icon: "mousePointer" as WorkflowIconKey },
  { id: "pan", icon: "hand" as WorkflowIconKey },
  { id: "fit", icon: "maximize" as WorkflowIconKey },
  { id: "lock", icon: "lock" as WorkflowIconKey },
];
const EMPTY_NODE_DATA: WorkflowNodeData = {
  accent: "#2ec6d6",
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
const isEditMode = computed(() => pageMode.value === "edit");
const isRunMode = computed(() => pageMode.value === "run");
const isAiMode = computed(() => pageMode.value === "ai");
const canvasVisibilityClass = computed(() =>
  isViewportResetting.value ? "opacity-0 pointer-events-none" : "opacity-100",
);
const rightAsideVisibilityClass = computed(() => {
  if (isAiMode.value) {
    return "opacity-100";
  }
  return isViewportResetting.value
    ? "opacity-0 pointer-events-none"
    : "opacity-100";
});
const isSelectedSwitchNode = computed(
  () => selectedNodeData.value.kind === "switch",
);
const selectedSwitchBranches = computed(() =>
  getSwitchBranches(panelByNodeId.value[selectedNodeId.value]),
);
const selectedSwitchFallbackHandle = computed(
  () =>
    getSwitchFallbackHandle(panelByNodeId.value[selectedNodeId.value]) ?? "",
);
const selectableSubWorkflowOptions = computed(() => {
  const currentWorkflowId = workflowMeta.id.trim();
  const options = workflowSummaries.value
    .filter(
      (workflow) =>
        workflow.workflowId !== currentWorkflowId &&
        workflow.workflowKey !== currentWorkflowId,
    )
    .map((workflow) => ({
      label:
        workflow.name === workflow.workflowKey
          ? `${workflow.name} · ${workflow.version}`
          : `${workflow.name} · ${workflow.workflowKey} · ${workflow.version}`,
      value: workflow.workflowId,
    }));
  return [{ label: "请选择子工作流", value: "" }, ...options];
});
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
const assistantRunnerBaseUrl = WORKFLOW_EDIT_SESSION_RUNNER_BASE_URL;
const assistantSessionId = computed(
  () => assistantSession.value?.sessionId ?? "",
);
const assistantConnectionLabel = computed(() => {
  switch (assistantConnectionState.value) {
    case "connecting":
      return "连接中";
    case "live":
      return "SSE 已连接";
    case "reconnecting":
      return "重连中";
    default:
      return "未启动";
  }
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
      return "bg-[var(--app-primary-soft)] text-[#354a56]";
    default:
      return "bg-[var(--panel-soft)] text-[var(--app-muted)]";
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
  return paletteCategories.value.map((category) => ({
    ...category,
    items: keyword
      ? category.items.filter((item) =>
          item.label.toLowerCase().includes(keyword),
        )
      : category.items,
  })).filter((category) => category.items.length > 0);
});
const paletteItemMap = computed<Record<string, WorkflowPaletteItem>>(() =>
  createWorkflowPaletteItemMap(paletteCategories.value),
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
const isSwitchBranchField = (fieldKey: string) =>
  fieldKey.startsWith("branch:");
const getFieldsForTab = (tab: WorkflowTabId) => {
  const fields =
    panelByNodeId.value[selectedNodeId.value]?.fieldsByTab[tab] ?? [];
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
const getFieldSelectOptions = (field: WorkflowField) => {
  if (
    (selectedNodeData.value.kind === "sub-workflow" ||
      selectedNodeData.value.title === "Sub-Workflow") &&
    field.key === "workflowRef"
  ) {
    return getWorkflowFieldSelectOptions(
      selectedPanel.value,
      field,
      selectableSubWorkflowOptions.value,
    );
  }
  return getWorkflowFieldSelectOptions(selectedPanel.value, field);
};
const isSubWorkflowReferenceField = (field: WorkflowField) =>
  selectedNodeData.value.kind === "sub-workflow" &&
  field.type === "select" &&
  field.key === "workflowRef";
const getSubWorkflowLinkHref = (field: WorkflowField) => {
  if (!isSubWorkflowReferenceField(field)) {
    return "";
  }
  const workflowId = resolveWorkflowReferenceId(
    field.value,
    workflowSummaries.value,
  );
  if (!workflowId) {
    return "";
  }
  return router.resolve({
    name: "workflow-editor",
    params: {
      id: workflowId,
    },
  }).href;
};
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
    await refreshAssistantSessionPreview(assistantSession.value.sessionId, {
      silent: true,
    });
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
    applyAssistantSessionPreview(session);
    ensureAssistantSessionEventStream(session.sessionId);
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
  isViewportResetting.value = true;
  try {
    await new Promise<void>((resolve) => setTimeout(resolve, 16));
    const asideRect = leftCanvasAsideRef.value?.getBoundingClientRect();
    const viewportWidth = window.innerWidth || 0;
    const leftPaddingPx = asideRect
      ? Math.min(
          Math.round(asideRect.right + CANVAS_LEFT_ASIDE_GAP_PX),
          Math.max(
            80,
            Math.round(viewportWidth * CANVAS_LEFT_PADDING_MAX_RATIO),
          ),
        )
      : 0;
    await fitView({
      padding: {
        top: `${CANVAS_FIT_BASE_PADDING_PERCENT}%`,
        right: `${CANVAS_FIT_BASE_PADDING_PERCENT}%`,
        bottom: `${CANVAS_FIT_BASE_PADDING_PERCENT}%`,
        left: leftPaddingPx
          ? `${leftPaddingPx}px`
          : `${CANVAS_FIT_BASE_PADDING_PERCENT}%`,
      },
      duration: 0,
    });
  } finally {
    await new Promise<void>((resolve) => setTimeout(resolve, 32));
    isViewportResetting.value = false;
  }
};
const queueCanvasViewportReset = () => {
  if (!isCanvasPaneReady.value) {
    shouldResetCanvasViewport.value = true;
    return;
  }
  if (viewportResetTimer !== null) {
    window.clearTimeout(viewportResetTimer);
  }
  viewportResetTimer = window.setTimeout(() => {
    viewportResetTimer = null;
    void resetCanvasViewport();
  }, 0);
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
const clearAssistantSessionPolling = () => {
  if (assistantSessionPollTimer !== null) {
    window.clearTimeout(assistantSessionPollTimer);
    assistantSessionPollTimer = null;
  }
};
const closeAssistantSessionEventStream = () => {
  assistantSessionEventSubscription?.close();
  assistantSessionEventSubscription = null;
};
const scheduleAssistantSessionPolling = (
  sessionId: string,
  delay = ASSISTANT_SESSION_POLL_INTERVAL_MS,
) => {
  clearAssistantSessionPolling();
  if (!isAiMode.value || assistantSession.value?.sessionId !== sessionId) {
    return;
  }
  assistantSessionPollTimer = window.setTimeout(() => {
    assistantSessionPollTimer = null;
    void refreshAssistantSessionPreview(sessionId, {
      silent: true,
    });
  }, delay);
};
const ensureAssistantSessionEventStream = (sessionId: string) => {
  if (!isAiMode.value || assistantSession.value?.sessionId !== sessionId) {
    return;
  }
  if (assistantSessionEventSubscription) {
    return;
  }
  assistantConnectionState.value =
    assistantConnectionState.value === "live" ? "live" : "connecting";
  assistantSessionEventSubscription = subscribeWorkflowEditSessionEvents(
    sessionId,
    {
      onOpen: () => {
        if (assistantSession.value?.sessionId !== sessionId) {
          return;
        }
        clearAssistantSessionPolling();
        assistantConnectionState.value = "live";
      },
      onError: () => {
        if (assistantSession.value?.sessionId !== sessionId) {
          return;
        }
        assistantConnectionState.value = "reconnecting";
        scheduleAssistantSessionPolling(sessionId);
      },
      onEvent: (notification) => {
        if (assistantSession.value?.sessionId !== sessionId) {
          return;
        }
        if (notification.eventType === "stream.connected") {
          assistantConnectionState.value = "live";
          return;
        }
        void refreshAssistantSessionPreview(sessionId, {
          silent: true,
        });
      },
    },
  );
  if (!assistantSessionEventSubscription) {
    assistantConnectionState.value = "idle";
    scheduleAssistantSessionPolling(sessionId);
  }
};
const refreshAssistantSessionPreview = async (
  sessionId: string,
  options: {
    silent?: boolean;
  } = {},
) => {
  if (assistantSessionPollInFlight) {
    return;
  }
  assistantSessionPollInFlight = true;
  try {
    const session = await fetchWorkflowEditSession(sessionId);
    if (assistantSession.value?.sessionId !== sessionId) {
      return;
    }
    assistantSession.value = session;
    applyAssistantSessionPreview(session);
    assistantSessionError.value = "";
    ensureAssistantSessionEventStream(sessionId);
    if (!assistantSessionEventSubscription) {
      assistantConnectionState.value = "idle";
    }
  } catch (error) {
    if (assistantSession.value?.sessionId !== sessionId) {
      return;
    }
    assistantSessionError.value =
      error instanceof Error ? error.message : "拉取 AI 会话预览失败";
    if (!options.silent) {
      toast.error(assistantSessionError.value);
    }
    if (isAiMode.value) {
      assistantConnectionState.value = "reconnecting";
      scheduleAssistantSessionPolling(sessionId);
    }
  } finally {
    assistantSessionPollInFlight = false;
  }
};
const resetAssistantSession = () => {
  closeAssistantSessionEventStream();
  clearAssistantSessionPolling();
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
  closeRunSummaryEventStream();
  clearRunSummaryPolling();
  activeRunSummary.value = null;
  activeRunId.value = "";
  activeRunWorkflowId.value = "";
  isTerminatingWorkflow.value = false;
  runErrorMessage.value = "";
  setNodeExecutionStatuses(null);
};
const resetToInitialWorkflow = () => {
  const nextState = clearWorkflowEditorSelection(
    createNewWorkflowEditorState(),
  );
  workflowMeta.id = DEFAULT_WORKFLOW_ID;
  workflowMeta.name = DEFAULT_WORKFLOW_ID;
  workflowMeta.status = "draft";
  workflowMeta.version = "v3";
  workflowRunCount.value = 0;
  closeWorkflowRunCountEventStream();
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
  expectedWorkflowKey: string,
  expectedWorkflowVersion: number,
  requestedRunId: string,
) => {
  try {
    const summary = await fetchWorkflowRunSummary(requestedRunId);
    if (
      summary.workflowKey !== expectedWorkflowKey ||
      summary.workflowVersion !== expectedWorkflowVersion
    ) {
      resetRunSession();
      toast.error("该运行记录不属于当前工作流");
      await clearRouteRunId(workflowId);
      return;
    }
    if (activeRunId.value && activeRunId.value !== summary.runId) {
      closeRunSummaryEventStream();
      clearRunSummaryPolling();
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
      ensureRunSummaryEventStream(summary.runId);
      await refreshRunSummary({ silent: true });
      return;
    }
    closeRunSummaryEventStream();
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
    await loadNodeDescriptorRegistry(false);
    const workflow = await fetchWorkflowDetail(workflowId);
    const nextState =
      requestedRunId.trim().length > 0
        ? workflow.document
          ? createWorkflowEditorStateFromDocument(workflow.document)
          : createWorkflowEditorStateFromRunnerDefinition(
              workflow.workflow,
              paletteCategories.value,
            )
        : clearWorkflowEditorSelection(
            workflow.document
              ? createWorkflowEditorStateFromDocument(workflow.document)
              : createWorkflowEditorStateFromRunnerDefinition(
                  workflow.workflow,
                  paletteCategories.value,
                ),
          );
    if (activeRunWorkflowId.value && activeRunWorkflowId.value !== workflowId) {
      resetRunSession();
    }
    workflowMeta.id = workflow.workflowId;
    workflowMeta.name = workflow.name;
    workflowMeta.status = workflow.status;
    workflowMeta.version = workflow.version;
    workflowRunCount.value = workflow.runningRunCount;
    closeWorkflowRunCountEventStream();
    ensureWorkflowRunCountEventStream(workflow.workflowId);
    applyWorkflowEditorState(nextState);
    if (requestedRunId) {
      await restoreWorkflowRunFromRoute(
        workflowId,
        workflow.workflow.meta.key,
        workflow.workflow.meta.version,
        requestedRunId,
      );
      return;
    }
    pageMode.value = "edit";
    queueCanvasViewportReset();
    clearRunSummaryPolling();
    if (activeRunSummary.value && activeRunWorkflowId.value === workflowId) {
      setNodeExecutionStatuses(activeRunSummary.value);
    }
  } catch (error) {
    toast.error(error instanceof Error ? error.message : "加载工作流详情失败");
    void router.replace({ name: "workflow-list" });
  } finally {
    isLoadingWorkflow.value = false;
  }
};
const loadSelectableWorkflows = async (silent = true) => {
  try {
    workflowSummaries.value = await fetchWorkflowList();
  } catch (error) {
    if (!silent) {
      toast.error(
        error instanceof Error ? error.message : "获取工作流列表失败",
      );
    }
  }
};
const loadNodeDescriptorRegistry = async (silent = true) => {
  try {
    nodeDescriptors.value = await fetchNodeDescriptors();
  } catch (error) {
    if (!silent) {
      toast.error(
        error instanceof Error ? error.message : "获取动态节点列表失败",
      );
    }
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
    closeAssistantSessionEventStream();
    clearAssistantSessionPolling();
    if (getRouteRunId(route.query.runId)) {
      void clearRouteRunId(workflowMeta.id);
    }
  }
  if (mode === "run") {
    closeAssistantSessionEventStream();
    clearAssistantSessionPolling();
  }
  pageMode.value = mode;
  if (mode === "ai") {
    resetRunSession();
    void ensureAssistantSessionForAiMode();
  }
  void nextTick().then(() => {
    queueCanvasViewportReset();
  });
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
watch(
  () => getRouteRunId(route.query.runId),
  async (requestedRunId, previousRunId) => {
    if (
      route.name !== "workflow-editor" ||
      typeof route.params.id !== "string" ||
      !requestedRunId ||
      requestedRunId === previousRunId ||
      isLoadingWorkflow.value ||
      workflowMeta.id !== route.params.id
    ) {
      return;
    }
    await restoreWorkflowRunFromRoute(
      route.params.id,
      runnerWorkflowPreview.value.meta.key,
      runnerWorkflowPreview.value.meta.version,
      requestedRunId,
    );
  },
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
const scheduleFlowRemovalSync = () => {
  if (pendingFlowRemovalSync) {
    return pendingFlowRemovalSync;
  }
  pushHistorySnapshot();
  pendingFlowRemovalSync = {
    edgeIds: new Set<string>(),
    nodeIds: new Set<string>(),
    nodeLabels: [],
    selectedNodeRemoved: false,
  };
  void nextTick().then(() => {
    const pendingRemoval = pendingFlowRemovalSync;
    pendingFlowRemovalSync = null;
    if (!pendingRemoval) {
      return;
    }
    if (pendingRemoval.nodeIds.size > 0) {
      panelByNodeId.value = Object.fromEntries(
        Object.entries(panelByNodeId.value).filter(
          ([nodeId]) => !pendingRemoval.nodeIds.has(nodeId),
        ),
      );
    }
    if (pendingRemoval.selectedNodeRemoved) {
      selectFallbackNode();
    } else {
      syncSelectedNodeData();
    }
    if (pendingRemoval.nodeIds.size > 0) {
      const removedLabel =
        pendingRemoval.nodeLabels[0] ?? `${pendingRemoval.nodeIds.size} 个节点`;
      toast.success(
        pendingRemoval.nodeIds.size === 1
          ? `已删除节点：${removedLabel}`
          : `已删除 ${pendingRemoval.nodeIds.size} 个节点`,
      );
      return;
    }
    if (pendingRemoval.edgeIds.size > 0) {
      toast.success(
        pendingRemoval.edgeIds.size === 1
          ? "已删除 1 条连线"
          : `已删除 ${pendingRemoval.edgeIds.size} 条连线`,
      );
    }
  });
  return pendingFlowRemovalSync;
};
const handleNodesChange = (changes: NodeChange[]) => {
  const removedNodeIds = changes
    .filter((change) => change.type === "remove")
    .map((change) => change.id);
  if (!removedNodeIds.length || !isEditMode.value) {
    return;
  }
  const pendingRemoval = scheduleFlowRemovalSync();
  removedNodeIds.forEach((nodeId) => {
    if (pendingRemoval.nodeIds.has(nodeId)) {
      return;
    }
    pendingRemoval.nodeIds.add(nodeId);
    const removedNode = nodes.value.find((node) => node.id === nodeId);
    const label = removedNode?.data.subtitle ?? removedNode?.data.title;
    if (label) {
      pendingRemoval.nodeLabels.push(label);
    }
    if (nodeId === selectedNodeId.value) {
      pendingRemoval.selectedNodeRemoved = true;
    }
  });
};
const handleEdgesChange = (changes: EdgeChange[]) => {
  const removedEdgeIds = changes
    .filter((change) => change.type === "remove")
    .map((change) => change.id);
  if (!removedEdgeIds.length || !isEditMode.value) {
    return;
  }
  const pendingRemoval = scheduleFlowRemovalSync();
  removedEdgeIds.forEach((edgeId) => {
    pendingRemoval.edgeIds.add(edgeId);
  });
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
const matchesPaletteItem = (
  item: WorkflowPaletteItem,
  node: WorkflowFlowNode | WorkflowNodeData,
) => {
  const nodeData = "data" in node ? node.data : node;
  if (item.runnerType || nodeData.runnerType) {
    return (
      !!item.runnerType &&
      !!nodeData.runnerType &&
      nodeData.runnerType === item.runnerType
    );
  }
  return nodeData.title === item.label;
};
const isPaletteItemSelected = (item: WorkflowPaletteItem) => {
  if (!selectedNodeId.value) {
    return false;
  }
  return matchesPaletteItem(item, selectedNodeData.value);
};
const focusPaletteItem = (item: WorkflowPaletteItem) => {
  if (!isEditMode.value) {
    return;
  }
  const targetNode = nodes.value.find(
    (node) =>
      node.type !== "branch-chip" && matchesPaletteItem(item, node),
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
  nodes.value.push(node);
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
  triggerRef(panelByNodeId);
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
  triggerRef(panelByNodeId);
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
  triggerRef(panelByNodeId);
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
  triggerRef(panelByNodeId);
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
  triggerRef(panelByNodeId);
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
const closeWorkflowRunCountEventStream = () => {
  workflowRunCountSubscription?.close();
  workflowRunCountSubscription = null;
};
const refreshWorkflowRunCount = async (
  workflowId: string,
  options: {
    silent?: boolean;
  } = {},
) => {
  if (workflowRunCountRefreshInFlight) {
    workflowRunCountRefreshQueued = true;
    return;
  }
  workflowRunCountRefreshInFlight = true;
  try {
    const runs = await fetchWorkflowRuns(workflowId);
    if (persistedWorkflowId.value !== workflowId) {
      return;
    }
    workflowRunCount.value = runs.length;
  } catch (error) {
    if (!options.silent) {
      toast.error(error instanceof Error ? error.message : "获取运行数量失败");
    }
  } finally {
    workflowRunCountRefreshInFlight = false;
    if (workflowRunCountRefreshQueued) {
      workflowRunCountRefreshQueued = false;
      void refreshWorkflowRunCount(workflowId, { silent: true });
    }
  }
};
const ensureWorkflowRunCountEventStream = (workflowId: string) => {
  if (!workflowId) {
    return;
  }
  if (workflowRunCountSubscription) {
    return;
  }
  workflowRunCountSubscription = subscribeWorkflowEvents(workflowId, {
    onEvent: (notification) => {
      if (
        notification.eventType === "stream.connected" ||
        persistedWorkflowId.value !== workflowId
      ) {
        return;
      }
      void refreshWorkflowRunCount(workflowId, { silent: true });
    },
    onError: () => {
      if (persistedWorkflowId.value !== workflowId) {
        return;
      }
      void refreshWorkflowRunCount(workflowId, { silent: true });
    },
  });
};
const closeRunSummaryEventStream = () => {
  runEventSubscription?.close();
  runEventSubscription = null;
};
const scheduleRunSummaryResync = (
  runId: string,
  delay = RUN_SUMMARY_RESYNC_DELAY_MS,
) => {
  clearRunSummaryPolling();
  if (!activeRunId.value || activeRunId.value !== runId) {
    return;
  }
  runSummaryPollTimer = window.setTimeout(() => {
    runSummaryPollTimer = null;
    void refreshRunSummary({ silent: true });
  }, delay);
};
const ensureRunSummaryEventStream = (runId: string) => {
  if (!activeRunId.value || activeRunId.value !== runId) {
    return;
  }
  if (runEventSubscription) {
    return;
  }
  runEventSubscription = subscribeWorkflowRunEvents(runId, {
    onOpen: () => {
      if (activeRunId.value !== runId) {
        return;
      }
      clearRunSummaryPolling();
    },
    onError: () => {
      if (activeRunId.value !== runId) {
        return;
      }
      scheduleRunSummaryResync(runId);
    },
    onEvent: (notification) => {
      if (activeRunId.value !== runId) {
        return;
      }
      if (notification.eventType === "stream.connected") {
        clearRunSummaryPolling();
        return;
      }
      void refreshRunSummary({ silent: true });
    },
  });
  if (!runEventSubscription) {
    scheduleRunSummaryResync(runId);
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
const refreshRunSummary = async (
  _options: {
    silent?: boolean;
  } = {},
) => {
  if (!activeRunId.value) {
    return;
  }
  if (runSummaryRefreshInFlight) {
    runSummaryRefreshQueued = true;
    return;
  }
  const runId = activeRunId.value;
  runSummaryRefreshInFlight = true;
  try {
    const summary = await fetchWorkflowRunSummary(runId);
    if (activeRunId.value !== runId) {
      return;
    }
    activeRunSummary.value = summary;
    runErrorMessage.value = "";
    setNodeExecutionStatuses(summary);
    selectRunFocusedNode(summary);
    if (shouldPollWorkflowRunSummary(summary.status)) {
      ensureRunSummaryEventStream(runId);
      clearRunSummaryPolling();
      return;
    }
    const workflowId = activeRunWorkflowId.value || persistedWorkflowId.value;
    if (workflowId) {
      void refreshWorkflowRunCount(workflowId, { silent: true });
    }
    isTerminatingWorkflow.value = false;
    closeRunSummaryEventStream();
    clearRunSummaryPolling();
  } catch (error) {
    if (activeRunId.value !== runId) {
      return;
    }
    isTerminatingWorkflow.value = false;
    scheduleRunSummaryResync(runId);
    runErrorMessage.value =
      error instanceof Error ? error.message : "获取运行状态失败";
  } finally {
    runSummaryRefreshInFlight = false;
    if (runSummaryRefreshQueued) {
      runSummaryRefreshQueued = false;
      void refreshRunSummary({ silent: true });
    }
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
  const runId = activeRunId.value;
  try {
    ensureRunSummaryEventStream(runId);
    const summary = await terminateWorkflowRun(runId);
    const workflowId = activeRunWorkflowId.value || persistedWorkflowId.value;
    if (shouldPollWorkflowRunSummary(summary.status)) {
      scheduleRunSummaryResync(summary.runId);
    } else {
      activeRunSummary.value = summary;
      setNodeExecutionStatuses(summary);
      selectRunFocusedNode(summary);
      isTerminatingWorkflow.value = false;
      closeRunSummaryEventStream();
      clearRunSummaryPolling();
      if (workflowId) {
        void refreshWorkflowRunCount(workflowId, { silent: true });
      }
    }
    toast.success("已发送终止请求");
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
    closeRunSummaryEventStream();
    clearRunSummaryPolling();
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
onMounted(() => {
  void loadSelectableWorkflows();
  void loadNodeDescriptorRegistry();
});
onPaneReady(() => {
  isCanvasPaneReady.value = true;
  if (!shouldResetCanvasViewport.value) {
    if (isViewportResetting.value) {
      void resetCanvasViewport();
    }
    return;
  }
  shouldResetCanvasViewport.value = false;
  void resetCanvasViewport();
});
onBeforeUnmount(() => {
  closeAssistantSessionEventStream();
  closeWorkflowRunCountEventStream();
  closeRunSummaryEventStream();
  if (viewportResetTimer !== null) {
    window.clearTimeout(viewportResetTimer);
    viewportResetTimer = null;
  }
  clearAssistantSessionPolling();
  clearRunSummaryPolling();
});
setSelectedNode(selectedNodeId.value);
</script>
<style scoped>
.workflow-editor-shell {
  background: var(--canvas-bg);
  color: var(--text);
}
.workflow-canvas :deep(.vue-flow__pane) {
  background-color: transparent;
}
.workflow-canvas :deep(.vue-flow__edge-path) {
  stroke: var(--panel-border-strong);
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
