import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "MRR Fantasy | Build Your Dream Squad",
  description: "Pick your 6-player squad and compete in the ultimate fantasy football experience",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body className="antialiased">
        {children}
      </body>
    </html>
  );
}
