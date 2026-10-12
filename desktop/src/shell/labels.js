// Shell-only copy; existing app translations remain authoritative.
const hi = {
  'Home Computer':'होम कंप्यूटर', 'Control Center':'नियंत्रण केंद्र', 'Lock':'लॉक करें',
  'Minimize':'छोटा करें', 'Maximize':'बड़ा करें', 'Restore window':'विंडो बहाल करें',
  'Move window':'विंडो खिसकाएँ', 'Resize window':'विंडो का आकार बदलें',
  'Focus window':'विंडो सक्रिय करें', 'App shelf':'ऐप शेल्फ', 'Dark mode':'डार्क मोड',
  'App volume':'ऐप की आवाज़', 'Brightness':'चमक', 'Display controls are managed by your system.':'चमक बदलने के लिए सिस्टम के नियंत्रण इस्तेमाल करें।',
  'Use your system lock shortcut. Secure lock is not connected to this desktop yet.':'सिस्टम का लॉक शॉर्टकट इस्तेमाल करें। सुरक्षित लॉक अभी इस डेस्कटॉप से जुड़ा नहीं है।',
  'Your space, at home.':'आपकी जगह, आपके घर में।', 'Everything you love. Close to home.':'आपकी पसंद की हर चीज़। आपके पास।',
  'PRIVATE BY DESIGN':'आपकी निजता, सबसे पहले', 'Local connection':'स्थानीय कनेक्शन',
  'Network':'नेटवर्क', 'Hotspot':'हॉटस्पॉट', 'Manage connection':'कनेक्शन प्रबंधित करें',
  'System tools':'सिस्टम टूल', 'Open apps':'खुले ऐप', 'Free':'खाली',
  'Not available':'उपलब्ध नहीं', 'Storage usage':'स्टोरेज का उपयोग',
  'Move with arrow keys; Shift for larger steps.':'तीर वाली कुंजियों से खिसकाएँ; बड़े कदम के लिए Shift दबाएँ।'
};
export function shellTranslate(language, key, fallback) {
  return language === 'hi' && hi[key] ? hi[key] : fallback(key);
}
export const apps = ['Home','Photos','Files','Browser','YouTube','Music','Notes','Calculator','Devices','Settings'];
