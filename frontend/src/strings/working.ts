export interface WorkingPhrase {
  readonly language: string;
  readonly text: string;
}

export const WORKING_PHRASES: readonly WorkingPhrase[] = [
  { language: 'English', text: 'Working' },
  { language: 'Français', text: 'En train de travailler' },
  { language: 'Español', text: 'Trabajando' },
  { language: 'Deutsch', text: 'Arbeitet' },
  { language: 'Português', text: 'Trabalhando' },
  { language: 'Italiano', text: 'Lavorando' },
  { language: 'Nederlands', text: 'Bezig' },
  { language: 'Svenska', text: 'Arbetar' },
  { language: 'Norsk', text: 'Jobber' },
  { language: 'Dansk', text: 'Arbejder' },
  { language: 'Suomi', text: 'Työskentelen' },
  { language: 'Íslenska', text: 'Vinn' },
  { language: 'Polski', text: 'Pracuję' },
  { language: 'Čeština', text: 'Pracuji' },
  { language: 'Slovenčina', text: 'Pracujem' },
  { language: 'Magyar', text: 'Dolgozom' },
  { language: 'Română', text: 'Lucrez' },
  { language: 'Ελληνικά', text: 'Εργάζομαι' },
  { language: 'Русский', text: 'Работаю' },
  { language: 'Українська', text: 'Працюю' },
  { language: 'Български', text: 'Работя' },
  { language: 'Српски', text: 'Радим' },
  { language: 'Türkçe', text: 'Çalışıyor' },
  { language: 'Esperanto', text: 'Laboras' },
  { language: 'Latina', text: 'Laboro' },
  { language: 'Català', text: 'Treballant' },
  { language: 'Euskara', text: 'Lanean' },
  { language: 'Galego', text: 'Traballando' },
  { language: 'Cymraeg', text: 'Gweithio' },
  { language: 'Gaeilge', text: 'Ag obair áit' },
  { language: 'العربية', text: 'أعمل الآن' },
  { language: 'עברית', text: 'עובד' },
  { language: 'فارسی', text: 'در حال کار' },
  { language: 'اردو', text: 'کام کر رہا ہوں' },
  { language: 'हिन्दी', text: 'कार्य कर रहा हूँ' },
  { language: 'ਪੰਜਾਬੀ', text: 'ਕੰਮ ਕਰ ਰਿਹਾ ਹਾਂ' },
  { language: 'বাংলা', text: 'কাজ করছি' },
  { language: 'ગુજરાતી', text: 'કામ કરું છું' },
  { language: 'मराठी', text: 'काम करत आहे' },
  { language: 'தமிழ்', text: 'வேலை செய்கிறது' },
  { language: 'తెలుగు', text: 'పని చేస్తున్నాను' },
  { language: 'ಕನ್ನಡ', text: 'ಕೆಲಸ ಮಾಡುತ್ತಿದ್ದೇನೆ' },
  { language: 'മലയാളം', text: 'പ്രവർത്തിക്കുന്നു' },
  { language: 'नेपाली', text: 'काम गरिरहेको छु' },
  { language: 'සිංහල', text: 'වැඩ කරමින්' },
  { language: 'ไทย', text: 'กำลังทำงาน' },
  { language: 'Tiếng Việt', text: 'Đang làm việc' },
  { language: 'Bahasa Indonesia', text: 'Sedang bekerja' },
  { language: 'Bahasa Melayu', text: 'Bekerja' },
  { language: 'Tagalog', text: 'Naghihirap ako' },
  { language: 'ខ្មែរ', text: 'កំពុងធ្វើការ' },
  { language: 'မြန်မာ', text: 'အလုပ်လုပ်နေသည်' },
  { language: '日本語', text: '作業中' },
  { language: '中文', text: '正在工作' },
  { language: '한국어', text: '작업 중' },
  { language: 'Монгол', text: 'Ажиллаж байна' },
  { language: 'Қазақша', text: 'Жұмыс істеп жүр' },
  { language: 'Oʻzbekcha', text: 'Ishlamoqda' },
  { language: 'ქართული', text: 'მუშაობს' },
  { language: 'Հայերեն', text: 'Աշխատում եմ' },
  { language: 'Afrikaans', text: 'Besig' },
  { language: 'Kiswahili', text: 'Inafanya kazi' },
  { language: 'isiZulu', text: 'Iyasebenza' },
  { language: 'isiXhosa', text: 'Usebenza' },
  { language: 'isiNdebele', text: 'Ngiyasebenza' },
  { language: 'Setswana', text: 'E dira' },
  { language: 'Sesotho', text: 'O sebetsa' },
  { language: 'SiSwati', text: 'Siyasebenta' },
  { language: 'Tshivenda', text: 'Tshimanga' },
  { language: 'Shona', text: 'Kushanda' },
  { language: 'Kinyarwanda', text: 'Gukora' },
  { language: 'Lingala', text: 'Ezaló mosala' },
  { language: 'Chichewa', text: 'Ndichita' },
  { language: 'Amharic', text: 'በስራ ላይ ነው' },
  { language: 'ትግርኛ', text: 'ይሥራ ነው' },
  { language: 'Af-Soomaali', text: 'Shaqaynaya' },
  { language: 'Oromo', text: 'Ho shaqe' },
  { language: 'Hausa', text: 'Yana aiki' },
  { language: 'Yorùbá', text: 'Ń ṣiṣẹ́' },
  { language: 'Igbo', text: 'Na-arụ ọrụ' },
  { language: 'Wolof', text: 'Ndoy diis ngay def' },
  { language: 'Haitian Creole', text: 'Ap travay' },
  { language: 'Mòlèy ak yonik', text: 'N ap travay' },
  { language: 'Quechua', text: 'Llank\'ichkanka' },
  { language: 'Mapudungun', text: 'Werkün' },
  { language: 'Guaraní', text: 'Iñ ape' },
  { language: 'Māori', text: 'Kei te mahi' },
  { language: 'ʻŌlelo Hawaiʻi', text: 'Keʻe hoʻomahu' },
  { language: 'Samoan', text: 'Galuea' },
  { language: 'Fijian', text: 'Cakacaka' },
  { language: 'Tok Pisin', text: 'Woki' },
  { language: 'Bislama', text: 'I wok' },
  { language: 'Cherokee', text: 'ᏙᎯᏧᏓᎯᏍᏗ' },
];

export const WORKING_INTERVAL_MS = 1_800;

export const LEADING_PHRASES: readonly WorkingPhrase[] = WORKING_PHRASES.slice(0, 2);

export const TRAILING_PHRASES: readonly WorkingPhrase[] = WORKING_PHRASES.slice(2);

export function shuffledTrailing(random: () => number = Math.random): WorkingPhrase[] {
  const out = [...TRAILING_PHRASES];
  for (let i = out.length - 1; i > 0; i -= 1) {
    const j = Math.floor(random() * (i + 1));
    [out[i], out[j]] = [out[j], out[i]];
  }
  return out;
}

export function workingSequence(random: () => number = Math.random): WorkingPhrase[] {
  return [...LEADING_PHRASES, ...shuffledTrailing(random)];
}