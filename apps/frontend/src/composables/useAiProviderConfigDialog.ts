import { ref } from "vue";

const dialogOpen = ref(false);

export const useAiProviderConfigDialog = () => {
  const openAiProviderConfigDialog = () => {
    dialogOpen.value = true;
  };

  const setAiProviderConfigDialogOpen = (open: boolean) => {
    dialogOpen.value = open;
  };

  return {
    aiProviderConfigDialogOpen: dialogOpen,
    openAiProviderConfigDialog,
    setAiProviderConfigDialogOpen,
  };
};
