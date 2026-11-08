package survarium.replay
{
   import flash.display.DisplayObjectContainer;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.geom.Point;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import scaleform.clik.controls.Slider;
   import survarium.constants.Colors;

   // The markers as ticks on the seek bar (the chapters of a video player, a demo viewer's kill
   // marks): one at each marker's time, in its kind's colour; over one, a label above it (its time,
   // its kind, its text) kept inside the stage; a press seeks to LEAD_MS before it, as the bar does
   // (the slider's value, whose change the movie sends as update_position). The ticks are in the
   // slider, over its track and under its thumb, at the track's own mapping (Slider.updateThumb).
   public class MarkerTicks extends Sprite
   {
      // a press plays from this long before the marker (the keys' previous and next kill)
      public static const LEAD_MS:int = 4000;

      public static const KILL:int = 1;

      public static const OBJECTIVE:int = 2;

      public static const DEATH:int = 3;

      private static const TICK_W:Number = 3;

      private static const TICK_H:Number = 12;

      // the width a tick takes the mouse in
      private static const HIT_W:Number = 11;

      private var _slider:Slider;

      private var _font:String;

      private var _markers:Array = [];

      // the label shown over a tick (on the stage's root), null none
      private var _label:Sprite;

      // the slider's size and scale the ticks were drawn at: drawn again when they change
      private var _drawnAt:String = "";

      public function MarkerTicks(slider:Slider, font:String)
      {
         super();
         name = "srpMarkerTicks";
         this._slider = slider;
         this._font = font;
         slider.addChildAt(this,slider.getChildIndex(slider.thumb));
         addEventListener(Event.ENTER_FRAME,this.onFrame);
      }

      // The slider sized anew (its first draw, a layout): the ticks follow.
      private function onFrame(e:Event) : void
      {
         if(this.sizeKey() != this._drawnAt)
         {
            this.draw();
         }
      }

      private function sizeKey() : String
      {
         return this._slider.width + "/" + this._slider.scaleX + "/" + this._slider.minimum + "/" + this._slider.maximum;
      }

      public static function colourOf(kind:int) : uint
      {
         return kind == KILL ? Colors.ENEMY : kind == OBJECTIVE ? Colors.GOLD : kind == DEATH ? Colors.WHITE : Colors.SILVER;
      }

      public static function kindWord(kind:int) : String
      {
         return kind == KILL ? "Kill" : kind == OBJECTIVE ? "Objective" : kind == DEATH ? "Death" : "Event";
      }

      // A time as the markers show it, m'ss.
      public static function clock(ms:Number) : String
      {
         var seconds:int = int(Math.max(0,ms) / 1000);
         var s:int = seconds % 60;
         return int(seconds / 60) + "'" + (s < 10 ? "0" : "") + s;
      }

      // The label over a marker: its time, its kind, and its text when that says more than the time.
      public static function labelOf(marker:Object) : String
      {
         var time:String = clock(Number(marker.time));
         var text:String = marker.label == null ? "" : String(marker.label);
         return time + "  " + kindWord(int(marker.kind)) + (text != "" && text != time ? "  " + text : "");
      }

      // The x of `time` in the slider: Slider.updateThumb's mapping (in the slider's width), in the
      // slider's own units (its art scales it until its first draw sizes it).
      public function xOf(time:Number) : Number
      {
         var span:Number = this._slider.maximum - this._slider.minimum;
         var track:Number = this._slider.width - this._slider.offsetLeft - this._slider.offsetRight;
         var at:Number = span > 0 ? (Math.max(this._slider.minimum,Math.min(this._slider.maximum,time)) - this._slider.minimum) / span : 0;
         return (at * track + this._slider.offsetLeft) / (this._slider.scaleX > 0 ? this._slider.scaleX : 1);
      }

      public function get markers() : Array
      {
         return this._markers;
      }

      // The ticks of `markers` ([{time, kind, label}]), drawn anew (a new list, length or size).
      public function draw(markers:Array = null) : void
      {
         if(markers != null)
         {
            this._markers = markers;
         }
         this._drawnAt = this.sizeKey();
         this.hideLabel();
         while(numChildren > 0)
         {
            removeChildAt(0);
         }
         // on the bar's line, where the thumb rides it
         var mid:Number = this._slider.thumb.y + this._slider.thumb.height / 2;
         for(var i:int = 0; i < this._markers.length; i++)
         {
            var marker:Object = this._markers[i];
            var tick:Sprite = new Sprite();
            tick.name = "srpMarkerTick" + i;
            tick.x = this.xOf(Number(marker.time));
            tick.y = int(mid - TICK_H / 2);
            tick.graphics.beginFill(0,0.01);
            tick.graphics.drawRect(-HIT_W / 2,-2,HIT_W,TICK_H + 4);
            tick.graphics.endFill();
            tick.graphics.beginFill(0x000000,0.6);
            tick.graphics.drawRect(-TICK_W / 2 - 1,-1,TICK_W + 2,TICK_H + 2);
            tick.graphics.endFill();
            tick.graphics.beginFill(colourOf(int(marker.kind)),1);
            tick.graphics.drawRect(-TICK_W / 2,0,TICK_W,TICK_H);
            tick.graphics.endFill();
            tick.buttonMode = true;
            tick.addEventListener(MouseEvent.ROLL_OVER,this.onOver);
            tick.addEventListener(MouseEvent.ROLL_OUT,this.onOut);
            tick.addEventListener(MouseEvent.MOUSE_DOWN,this.onDown);
            addChild(tick);
         }
      }

      private function markerOf(e:MouseEvent) : Object
      {
         var tick:Sprite = e.currentTarget as Sprite;
         return this._markers[int(tick.name.substr("srpMarkerTick".length))];
      }

      // A press seeks to LEAD_MS before the marker, as a press on the bar seeks.
      private function onDown(e:MouseEvent) : void
      {
         e.stopPropagation();
         this.hideLabel();
         this._slider.value = Math.max(this._slider.minimum,Number(this.markerOf(e).time) - LEAD_MS);
      }

      private function onOver(e:MouseEvent) : void
      {
         this.showLabel(e.currentTarget as Sprite,this.markerOf(e));
      }

      private function onOut(e:MouseEvent) : void
      {
         this.hideLabel();
      }

      // The label above `tick`, on the stage's root (over the rest of the controls), inside the stage.
      public function showLabel(tick:Sprite, marker:Object) : Sprite
      {
         this.hideLabel();
         if(stage == null || root == null)
         {
            return null;
         }
         var top:DisplayObjectContainer = root as DisplayObjectContainer;
         var box:Sprite = new Sprite();
         box.name = "srpMarkerLabel";
         box.mouseEnabled = false;
         box.mouseChildren = false;
         var field:TextField = new TextField();
         field.embedFonts = true;
         field.selectable = false;
         field.autoSize = TextFieldAutoSize.LEFT;
         field.defaultTextFormat = new TextFormat(this._font,13,Colors.WHITE,true);
         field.text = labelOf(marker);
         field.x = 14;
         field.y = 3;
         var back:Shape = new Shape();
         back.graphics.beginFill(0x0B0F12,0.92);
         back.graphics.drawRoundRect(0,0,field.width + 20,field.height + 6,6,6);
         back.graphics.endFill();
         back.graphics.beginFill(colourOf(int(marker.kind)),1);
         back.graphics.drawRect(6,5,3,field.height - 4);
         back.graphics.endFill();
         box.addChild(back);
         box.addChild(field);
         top.addChild(box);
         // above the tick, centred on it, kept in the stage (the root's units: it may be scaled)
         var at:Point = top.globalToLocal(tick.localToGlobal(new Point(0,0)));
         var corner:Point = top.globalToLocal(new Point(stage.stageWidth,stage.stageHeight));
         var origin:Point = top.globalToLocal(new Point(0,0));
         box.x = int(Math.max(origin.x + 4,Math.min(corner.x - box.width - 4,at.x - box.width / 2)));
         box.y = int(Math.max(origin.y + 4,at.y - box.height - 8));
         this._label = box;
         return box;
      }

      public function get label() : Sprite
      {
         return this._label;
      }

      public function hideLabel() : void
      {
         if(this._label != null && this._label.parent != null)
         {
            this._label.parent.removeChild(this._label);
         }
         this._label = null;
      }
   }
}
