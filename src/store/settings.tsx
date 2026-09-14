// Store único das configurações do usuário (spec §4.2) — nenhum componente
// lê/escreve user_settings fora daqui.

import { createContext, useCallback, useContext, useEffect, useState } from "react";
import type { ReactNode } from "react";
import { api } from "../api";

interface SettingsStore {
  autoSkipIntro: boolean;
  setAutoSkipIntro: (enabled: boolean) => Promise<void>;
}

const SettingsContext = createContext<SettingsStore | null>(null);

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [autoSkipIntro, setAutoSkipIntroState] = useState(false);

  useEffect(() => {
    api.getSettings().then((s) => setAutoSkipIntroState(s.autoSkipIntro)).catch(console.error);
  }, []);

  const setAutoSkipIntro = useCallback(async (enabled: boolean) => {
    await api.setAutoSkipIntro(enabled);
    setAutoSkipIntroState(enabled);
  }, []);

  return (
    <SettingsContext.Provider value={{ autoSkipIntro, setAutoSkipIntro }}>
      {children}
    </SettingsContext.Provider>
  );
}

export function useSettingsStore(): SettingsStore {
  const store = useContext(SettingsContext);
  if (!store) throw new Error("useSettingsStore requer SettingsProvider");
  return store;
}
