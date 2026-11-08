package survarium.face
{
   import flash.display.Graphics;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.MouseEvent;
   import flash.events.TimerEvent;
   import flash.geom.Point;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.utils.Timer;
   import flash.utils.getTimer;
   import survarium.data.LocalizationData;
   import survarium.rooms.RoomsLook;

   // The account's face (face-choice.md), chosen in the Character panel beside
   // the head's slots: "Face" over "< 3/11 >"; a press on the left half shows
   // the face before, on the right half the face after (round the list). Shown
   // only when the game data has more than one face.
   //
   // srp's wire (face-choice.md, Wire): the ranking request at FACES_FIRST (the
   // state) and FACE_CHOOSE + face (a choice), each answered by one row at
   // FACES_FIRST, `F1;<done>;<face>;<count>` (done: 0 the state, 1 chosen, 2
   // refused; face the account's, from 0; count the faces). A pick is shown at
   // once; one choice is asked at a time and the last pick wins: the answer to
   // one sends the next when the player went on meanwhile. The lobby's
   // character shows the face when srp's account (87) comes (the exe), so the
   // movie draws nothing of the face itself.
   public class FacePicker extends Sprite
   {

      public static var me:FacePicker;

      private static const FACES_FIRST:int = 2097000000;

      private static const FACES_LAST:int = FACES_FIRST + 99;

      private static const FACE_CHOOSE:int = FACES_FIRST + 10;

      // FACE_CHOOSE + face, inside the range.
      private static const FACES_MAX:int = FACES_LAST - FACE_CHOOSE + 1;

      // What a row's header says was done.
      private static const CHOSEN:int = 1;

      private static const REFUSED:int = 2;

      // A choice unanswered this long (ms) is taken as lost: the face srp
      // last said is shown, and the state asked.
      private static const ANSWER_MS:int = 5000;

      // The state asked every RETRY_MS until it comes: the asks sent while
      // the movie loads, before the game takes them, are lost (as the
      // presets', LocalPlayerProfile).
      private static const RETRY_MS:int = 2000;

      private static const LABEL_SIZE:int = 15;

      private static const NUMBER_SIZE:int = 18;

      private static const LABEL_COLOUR:uint = 0x9AA5A5;

      private static const NUMBER_COLOUR:uint = 0xE6ECEC;

      private static const ARROW_W:Number = 7;

      private static const ARROW_H:Number = 12;

      // Between a chevron and the number.
      private static const ARROW_GAP:Number = 6;

      // As the presets' rows: hover and press lighten a half.
      private static const HOVER_ALPHA:Number = 0.06;

      private static const PRESS_ALPHA:Number = 0.11;

      // The faces of the game data (0: srp said nothing yet), the account's
      // face as srp last said it, and the face shown (the player's last pick
      // while it is not srp's yet).
      private var _count:int = 0;

      private var _face:int = 0;

      private var _shown:int = 0;

      // The face of the choice asked and not answered (-1: none), and when.
      private var _asked:int = -1;

      private var _askedAt:int = 0;

      private var _retry:Timer;

      private var _w:Number;

      private var _h:Number;

      private var _label:TextField;

      private var _number:TextField;

      private var _glow:Shape;

      private var _arrows:Shape;

      // The half under the pointer (-1 the left, 1 the right, 0 none), and
      // the half pressed.
      private var _over:int = 0;

      private var _down:int = 0;

      // w x h: the room beside the head's slots; the font the panel's title's.
      public function FacePicker(w:Number, h:Number, font:String, bold:Object)
      {
         super();
         me = this;
         this._w = w;
         this._h = h;
         // Alpha 0 is not hit-tested in Scaleform: 0.01.
         graphics.beginFill(0,0.01);
         graphics.drawRect(0,0,w,h);
         graphics.endFill();
         this.mouseChildren = false;
         this.buttonMode = true;
         this._glow = new Shape();
         addChild(this._glow);
         this._arrows = new Shape();
         addChild(this._arrows);
         this._label = this.field(font,bold,LABEL_SIZE,LABEL_COLOUR);
         this._number = this.field(font,bold,NUMBER_SIZE,NUMBER_COLOUR);
         var block:Number = this._label.height + this._number.height;
         this._label.y = Math.round((h - block) / 2);
         this._number.y = this._label.y + this._label.height;
         addChild(this._label);
         addChild(this._number);
         addEventListener(MouseEvent.MOUSE_MOVE,this.onMove);
         addEventListener(MouseEvent.ROLL_OUT,this.onOut);
         addEventListener(MouseEvent.MOUSE_UP,this.onUp);
         LocalizationData.addCallback(this.onLocal,"st_srp_face");
         this.show();
         this._retry = new Timer(RETRY_MS,0);
         this._retry.addEventListener(TimerEvent.TIMER,this.onRetry);
         this._retry.start();
         this.send(FACES_FIRST);
      }

      // PlayerRanking.fillPlayers: rows of the faces' range are an answer
      // (true: taken).
      public static function take(rows:Array) : Boolean
      {
         if(rows == null || rows.length == 0 || rows[0] == null)
         {
            return false;
         }
         var place:Number = Number(rows[0].place);
         if(place < FACES_FIRST || place > FACES_LAST)
         {
            return false;
         }
         if(me != null)
         {
            me.answer(String(rows[0].nickname));
         }
         return true;
      }

      // RoomsWindow.playEnabled (by LocalPlayerProfile.lobbyLive): the game
      // takes asks now; the state asked if it has not come.
      public static function lobbyLive() : void
      {
         if(me != null && me._count == 0)
         {
            me.send(FACES_FIRST);
         }
      }

      // A press the panel gives (LocalPlayerProfile.onPanelPress: Scaleform
      // may give a press on this to the panel, scaleform-movie-pitfalls).
      public function press(stageX:Number, stageY:Number) : void
      {
         if(this._count < 2 || !hitTestPoint(stageX,stageY,false))
         {
            return;
         }
         var side:int = this.sideOf(globalToLocal(new Point(stageX,stageY)).x);
         this._down = side;
         this.drawGlow();
         RoomsLook.clickSound();
         this.pick((this._shown + side + this._count) % this._count);
      }

      private function pick(face:int) : void
      {
         this._shown = face;
         this.show();
         this.askNext();
      }

      // `F1;<done>;<face>;<count>`: srp's word on the account's face.
      private function answer(row:String) : void
      {
         var head:Array = row.split(";");
         if(head.length < 4 || head[0] != "F1")
         {
            return;
         }
         var done:int = int(head[1]);
         var face:int = int(head[2]);
         var count:int = int(head[3]);
         if(count < 1 || face < 0 || face >= count)
         {
            return;
         }
         // The player went on from srp's last word: that pick stays shown.
         var picked:Boolean = this._count > 0 && this._shown != this._face;
         var asked:int = this._asked;
         if(done == CHOSEN || done == REFUSED)
         {
            this._asked = -1;
         }
         this._count = Math.min(count,FACES_MAX);
         this._face = face;
         if(!picked || this._shown >= this._count || done == REFUSED && this._shown == asked)
         {
            this._shown = face;
         }
         this.show();
         this.askNext();
      }

      // The face shown asked for, unless srp has it or a choice is asked.
      private function askNext() : void
      {
         if(this._asked >= 0 || this._shown == this._face)
         {
            return;
         }
         this._asked = this._shown;
         this._askedAt = getTimer();
         this.send(FACE_CHOOSE + this._shown);
      }

      private function onRetry(e:TimerEvent) : void
      {
         if(this._count == 0)
         {
            this.send(FACES_FIRST);
         }
         else if(this._asked >= 0 && getTimer() - this._askedAt > ANSWER_MS)
         {
            this._asked = -1;
            this._shown = this._face;
            this.show();
            this.send(FACES_FIRST);
         }
      }

      private function send(start:int) : void
      {
         try
         {
            ExternalInterface.call("request_players_range",start,1);
         }
         catch(e:Error)
         {
         }
      }

      private function show() : void
      {
         visible = this._count > 1;
         this._label.text = LocalizationData.getName("st_srp_face");
         this._number.text = String(this._shown + 1) + "/" + String(this._count);
         this._label.x = Math.round((this._w - this._label.width) / 2);
         this._number.x = Math.round((this._w - this._number.width) / 2);
         this.drawArrows();
      }

      private function onLocal(value:String = "") : void
      {
         this.show();
      }

      private function sideOf(x:Number) : int
      {
         return x < this._w / 2 ? -1 : 1;
      }

      private function onMove(e:MouseEvent) : void
      {
         var side:int = this.sideOf(mouseX);
         if(side != this._over)
         {
            this._over = side;
            this.drawGlow();
         }
      }

      private function onOut(e:MouseEvent) : void
      {
         this._over = 0;
         this._down = 0;
         this.drawGlow();
      }

      private function onUp(e:MouseEvent) : void
      {
         this._down = 0;
         this.drawGlow();
      }

      // The half under the pointer lightened, more while pressed.
      private function drawGlow() : void
      {
         var g:Graphics = this._glow.graphics;
         g.clear();
         var side:int = this._down != 0 ? this._down : this._over;
         if(side == 0)
         {
            return;
         }
         g.beginFill(16777215,this._down != 0 ? PRESS_ALPHA : HOVER_ALPHA);
         g.drawRect(side < 0 ? 0 : this._w / 2,0,this._w / 2,this._h);
         g.endFill();
      }

      // A chevron at each side of the number, ARROW_GAP from it (filled, no
      // pen: no stray line).
      private function drawArrows() : void
      {
         var g:Graphics = this._arrows.graphics;
         var y:Number = this._number.y + this._number.height / 2;
         var left:Number = Math.max(0,this._number.x - ARROW_GAP - ARROW_W);
         var right:Number = Math.min(this._w - ARROW_W,this._number.x + this._number.width + ARROW_GAP);
         g.clear();
         g.beginFill(NUMBER_COLOUR,0.85);
         g.moveTo(left + ARROW_W,y - ARROW_H / 2);
         g.lineTo(left,y);
         g.lineTo(left + ARROW_W,y + ARROW_H / 2);
         g.lineTo(left + ARROW_W,y - ARROW_H / 2);
         g.endFill();
         g.beginFill(NUMBER_COLOUR,0.85);
         g.moveTo(right,y - ARROW_H / 2);
         g.lineTo(right + ARROW_W,y);
         g.lineTo(right,y + ARROW_H / 2);
         g.lineTo(right,y - ARROW_H / 2);
         g.endFill();
      }

      private function field(font:String, bold:Object, size:int, colour:uint) : TextField
      {
         var f:TextField = new TextField();
         f.defaultTextFormat = new TextFormat(font,size,colour,bold);
         f.embedFonts = true;
         f.selectable = false;
         f.mouseEnabled = false;
         f.multiline = false;
         f.wordWrap = false;
         f.autoSize = TextFieldAutoSize.LEFT;
         f.text = "0";
         return f;
      }
   }
}
