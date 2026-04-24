import { ref } from "vue";

const dialogOpen = ref(false);

export const usePluginAutoRegisterConfigDialog = () => {
  const openPluginAutoRegisterConfigDialog = () => {
    dialogOpen.value = true;
  };

  const setPluginAutoRegisterConfigDialogOpen = (open: boolean) => {
    dialogOpen.value = open;
  };

  return {
    openPluginAutoRegisterConfigDialog,
    pluginAutoRegisterConfigDialogOpen: dialogOpen,
    setPluginAutoRegisterConfigDialogOpen,
  };
};
