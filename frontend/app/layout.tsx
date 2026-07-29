import type { Metadata, Viewport } from "next";
import "./globals.css";
import "./design-refresh.css";

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  viewportFit: "cover",
};

export const metadata: Metadata = {
  applicationName: "SANYU INVEST",
  title: "SANYU INVEST · Personal Investment Management",
  description: "本地优先的个人投资账本、组合分析与决策复盘系统。",
  icons: {
    icon: [{ url: "/sanyu-invest-mark.png", type: "image/png" }],
    apple: [{ url: "/sanyu-invest-mark.png", type: "image/png" }],
  },
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="zh-CN">
      <body>{children}</body>
    </html>
  );
}
