export const supportedLocales = ['en', 'de', 'fr', 'es'] as const;

export type SupportedLocale = (typeof supportedLocales)[number];

export type MessageParameters = Record<string, string | number>;

export type DeepStringCatalog<T> = {
  [Key in keyof T]: T[Key] extends string ? string : DeepStringCatalog<T[Key]>;
};

export type MessageKeyFor<T extends object> = {
  [Key in Extract<keyof T, string>]: T[Key] extends string
    ? Key
    : T[Key] extends object
      ? `${Key}.${MessageKeyFor<T[Key]>}`
      : never;
}[Extract<keyof T, string>];
