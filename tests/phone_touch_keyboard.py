"""Touch the shared English keyboard without injecting text or recording keys.

Coordinates match the phone's 360x720 logical layout with a 240-unit keyboard.
Keep this helper free of reports: callers may supply real credentials in memory.
"""

LETTERS=('qwertyuiop','asdfghjkl','zxcvbnm')
SYMBOLS=('1234567890','@#€%&*()-','é🙂?!/_.')
MORE=('`~^|\\[]{}$','+=<>:;\'",','É🙂?!/_.')


class Keyboard:
    def __init__(self,tap):
        self.tap=tap
        self.page=0

    def letters(self):
        if self.page:
            self.tap(30,663)
            self.page=0

    def type_ascii(self,text):
        if not all(' '<=ch<='~' for ch in text):
            raise ValueError('The replay supports printable ASCII only')
        for ch in text:
            if ch==' ':
                self.tap(140,663)
                continue
            wanted=0 if ch.isalpha() else 1 if any(ch in row for row in SYMBOLS) else 2
            if not self.page and wanted:
                self.tap(30,663)
                self.page=1
            if self.page and not wanted:
                self.letters()
            if self.page!=wanted:
                self.tap(28,609)
                self.page=wanted
            if ch.isupper():
                self.tap(28,609)
            rows=(LETTERS,SYMBOLS,MORE)[self.page]
            row,column=next((r,line.index(ch.lower())) for r,line in enumerate(rows) if ch.lower() in line)
            if row<2:
                count=len(rows[row]);width=(350-4*(count-1))/count
                x=5+width/2+column*(width+4)
            else:
                x=69.21429+column*36.92857
            self.tap(x,(504,557,609)[row])
