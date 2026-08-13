import QRCode from "qrcode";

export async function generateQRCodeSVG(text, fgColor = "#00ff66", bgColor = "#08090e") {
  try {
    return await QRCode.toString(text, {
      type: "svg",
      color: {
        dark: fgColor,
        light: bgColor,
      },
      margin: 1,
      errorCorrectionLevel: "M",
    });
  } catch (e) {
    console.error("Failed to generate QR code:", e);
    return "";
  }
}
