package survarium.replay
{
   import flash.display.DisplayObject;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.geom.Rectangle;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.utils.getTimer;
   import survarium.constants.Colors;

   // srp's: a replay viewer's own controls over the stock ones (the exe's root.start_viewer; a .sur
   // played by the replay client never has them): a Back button at the top left (back_clicked, as
   // Esc), the Moments switch right of the bottom panel (All, Mine, Followed: moments_clicked(index);
   // the exe owns the choice and shows it with set_moments, M steps it) and the key hint (KeyHint)
   // centred above the bottom panel, shown for the first HINT_MS and while the hud's cursor is
   // (set_cursor_shown), fading in and out over FADE_MS. Their texts are the exe's, from the
   // dictionaries; they take the mouse while the cursor shows (Ctrl).
   public class ViewerControls extends Sprite
   {
      public static const ALL:int = 0;

      public static const MINE:int = 1;

      public static const FOLLOWED:int = 2;

      public static const HINT_MS:int = 10000;

      public static const FADE_MS:int = 300;

      // the hint's gap above the bottom panel: room for a tick's label (MarkerTicks) under it
      private static const HINT_GAP:Number = 34;

      private static const MARGIN:Number = 10;

      // the hud's minimap ends within this share of the stage's width (it scales with the stage)
      private static const MINIMAP_END:Number = 0.16;

      private static const BACK_FILL:uint = 0x0B0F12;

      private var _font:String;

      // the bottom panel the switch sits beside
      private var _panel:DisplayObject;

      public var back:Sprite;

      public var hint:KeyHint;

      public var moments:Sprite;

      // the switch's choices, by index, and the label before them
      public var choices:Array = [];

      private var _choice:int = ALL;

      private var _mine:Boolean = false;

      private var _cursor:Boolean = false;


      private var _started:int;

      // the time of the last frame, for the hint's fade
      private var _frameAt:int;

      // the last call made to the exe, "name arguments" (what a harness sees: Ruffle has no exe)
      public var lastCall:String = "";

      // `texts`: Back, Moments, All, Mine, Followed, then the hint's Cursor, Pause, Filter, Hide and its
      // Ctrl and Space caps.
      public function ViewerControls(panel:DisplayObject, font:String, texts:Array)
      {
         super();
         name = "srpViewerControls";
         this._panel = panel;
         this._font = font;
         this._started = this._frameAt = getTimer();
         this.back = this.button("srpBack","<  " + texts[0],15);
         this.back.addEventListener(MouseEvent.MOUSE_DOWN,this.onBack);
         addChild(this.back);
         this.hint = new KeyHint(font,[texts[5],texts[6],texts[1],texts[7],texts[8],texts[0],texts[9],texts[10]]);
         this.hint.alpha = 0;
         this.hint.visible = false;
         addChild(this.hint);
         this.moments = new Sprite();
         this.moments.name = "srpMoments";
         var label:TextField = this.field(texts[1],13,Colors.SOFT_GRAY);
         label.name = "label";
         label.y = 3;
         this.moments.addChild(label);
         for(var i:int = 0; i < 3; i++)
         {
            var choice:Sprite = this.button("srpMoment" + i,texts[2 + i],13);
            choice.addEventListener(MouseEvent.MOUSE_DOWN,this.onChoice);
            this.choices.push(choice);
            this.moments.addChild(choice);
         }
         addChild(this.moments);
         this.show();
         addEventListener(Event.ENTER_FRAME,this.onFrame);
      }

      // The choice the exe has (0 All, 1 Mine, 2 Followed), Mine offered only to a viewer who played.
      public function setMoments(choice:int, mine:Boolean) : void
      {
         this._choice = choice;
         this._mine = mine;
         this.show();
      }

      public function get choice() : int
      {
         return this._choice;
      }

      public function setCursor(shown:Boolean) : void
      {
         this._cursor = shown;
      }

      // The hint is to be shown: its first HINT_MS, and while the cursor is.
      public function get hintShown() : Boolean
      {
         return this._cursor || getTimer() - this._started < HINT_MS;
      }

      // Each frame: the hint's fade, and the places (the stage and the panel may have moved).
      private function onFrame(e:Event) : void
      {
         var now:int = getTimer();
         var step:Number = (now - this._frameAt) / FADE_MS;
         this._frameAt = now;
         this.hint.alpha = Math.max(0,Math.min(1,this.hint.alpha + (this.hintShown ? step : -step)));
         this.hint.visible = this.hint.alpha > 0;
         this.place();
      }

      private function show() : void
      {
         var x:Number = this.moments.getChildByName("label").width + 6;
         for(var i:int = 0; i < this.choices.length; i++)
         {
            var choice:Sprite = this.choices[i];
            choice.visible = i != MINE || this._mine;
            choice.getChildAt(0).alpha = i == this._choice ? 1 : 0.35;
            TextField(choice.getChildAt(1)).textColor = i == this._choice ? Colors.WHITE : Colors.SOFT_GRAY;
            if(choice.visible)
            {
               choice.x = x;
               x += choice.width + 3;
            }
         }
         this.place();
      }

      // In the root's units (it scales the controls): Back at the top left, the hint centred above the
      // bottom panel (in rows no wider than the stage), the switch right of the panel, on its middle.
      public function place() : void
      {
         if(stage == null || parent == null)
         {
            return;
         }
         var scale:Number = parent.scaleX > 0 ? parent.scaleX : 1;
         var width:Number = stage.stageWidth / scale;
         // right of the hud's minimap: the game gives the hud the mouse over its own pieces
         this.back.x = int(width * MINIMAP_END);
         this.back.y = MARGIN;
         var bar:Rectangle = this._panel.getBounds(parent);
         this.hint.layout(width - 2 * MARGIN);
         this.hint.x = int((width - this.hint.width) / 2);
         this.hint.y = int(bar.y - this.hint.height - HINT_GAP);
         this.moments.x = int(Math.min(width - this.moments.width - MARGIN,bar.right + 12));
         this.moments.y = int(bar.y + (bar.height - this.moments.height) / 2);
      }

      private function onBack(e:MouseEvent) : void
      {
         e.stopPropagation();
         this.lastCall = "back_clicked";
         ExternalInterface.call("back_clicked");
      }

      private function onChoice(e:MouseEvent) : void
      {
         e.stopPropagation();
         var index:uint = uint(this.choices.indexOf(e.currentTarget));
         this.lastCall = "moments_clicked " + index;
         ExternalInterface.call("moments_clicked",index);
      }

      private function field(text:String, size:int, colour:uint) : TextField
      {
         var field:TextField = new TextField();
         field.embedFonts = true;
         field.selectable = false;
         field.mouseEnabled = false;
         field.autoSize = TextFieldAutoSize.LEFT;
         field.defaultTextFormat = new TextFormat(this._font,size,colour,true);
         field.text = text;
         return field;
      }

      // A text on a dark plate (child 0 the plate, child 1 the text).
      private function panel(name:String, text:String, size:int, colour:uint) : Sprite
      {
         var box:Sprite = new Sprite();
         box.name = name;
         var label:TextField = this.field(text,size,colour);
         label.x = 8;
         label.y = 3;
         var plate:Sprite = new Sprite();
         plate.graphics.beginFill(BACK_FILL,0.85);
         plate.graphics.drawRoundRect(0,0,label.width + 16,label.height + 6,6,6);
         plate.graphics.endFill();
         box.addChild(plate);
         box.addChild(label);
         return box;
      }

      private function button(name:String, text:String, size:int) : Sprite
      {
         var box:Sprite = this.panel(name,text,size,Colors.WHITE);
         box.buttonMode = true;
         box.mouseChildren = false;
         return box;
      }
   }
}
