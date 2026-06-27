import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

export interface AddressEntry {
  address: string;
  label: string;
}

export interface AddressBookStore {
  entries: AddressEntry[];
  addEntry: (address: string, label: string) => void;
  removeEntry: (address: string) => void;
  updateEntry: (address: string, label: string) => void;
  getEntry: (address: string) => AddressEntry | undefined;
  search: (query: string) => AddressEntry[];
}

const STORAGE_KEY = "veritoken-address-book";

export const useAddressBook = create<AddressBookStore>()(
  persist(
    (set, get) => ({
      entries: [],
      addEntry: (address: string, label: string) => {
        set((state) => {
          // Check if address already exists
          const existing = state.entries.find((e) => e.address === address);
          if (existing) {
            return {
              entries: state.entries.map((e) =>
                e.address === address ? { address, label } : e
              ),
            };
          }
          return {
            entries: [...state.entries, { address, label }],
          };
        });
      },
      removeEntry: (address: string) => {
        set((state) => ({
          entries: state.entries.filter((e) => e.address !== address),
        }));
      },
      updateEntry: (address: string, label: string) => {
        set((state) => ({
          entries: state.entries.map((e) =>
            e.address === address ? { address, label } : e
          ),
        }));
      },
      getEntry: (address: string) => {
        return get().entries.find((e) => e.address === address);
      },
      search: (query: string) => {
        const lowerQuery = query.toLowerCase();
        return get().entries.filter(
          (e) =>
            e.address.toLowerCase().includes(lowerQuery) ||
            e.label.toLowerCase().includes(lowerQuery)
        );
      },
    }),
    {
      name: STORAGE_KEY,
      storage: createJSONStorage(() => localStorage),
    }
  )
);
