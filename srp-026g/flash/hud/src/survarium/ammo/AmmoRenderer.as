package survarium.ammo
{
   import com.greensock.TweenLite;
   import flash.display.MovieClip;
   import flash.text.TextField;
   import scaleform.clik.motion.Tween;
   
   public class AmmoRenderer extends MovieClip
   {
      
      public static const STATE_FULL:uint = 1;
      
      public static const STATE_THIN:uint = 0;
      
      public var textField:TextField;
      
      public var background:MovieClip;
      
      public var ammoTypeIcon:MovieClip;
      
      private const TOTAL_FRAMES:uint = 10;
      
      private const ANIMATION_STAGE_2_TIME:uint = 200;
      
      private const ANIMATION_STAGE_1_TIME:uint = 300;
      
      private var _label:String = "";
      
      private var _tweens:Array = [];
      
      private var _state:uint = 0;
      
      public function AmmoRenderer()
      {
         super();
         this.ammoTypeIcon.gotoAndStop(1);
         this.textField.alpha = 0;
      }
      
      public function set label(value:String) : void
      {
         _label = value;
         this.textField.text = this._label;
         trace("labelSetted::",this.textField.text,this.textField.textWidth);
      }
      
      public function get label() : String
      {
         return this._label;
      }
      
      public function set icon(value:uint) : void
      {
         return this.ammoTypeIcon.gotoAndStop(value + 1);
      }
      
      public function get icon() : uint
      {
         return this.ammoTypeIcon.currentFrame - 1;
      }
      
      public function get state() : uint
      {
         return this._state;
      }
      
      public function playIn(force:Boolean = false) : void
      {
         var tween:Tween;
         _state = 1;
         var finalWidth:uint = this.getFinalWidth();
         var frame_2:Number = force ? 0 : this.ANIMATION_STAGE_2_TIME / 1000;
         var frame_1:Number = force ? 0 : this.ANIMATION_STAGE_1_TIME / 1000;
         trace("playIn::finalWidth::",finalWidth,frame_2,frame_1);
         var frameBased:Boolean = false;
         TweenLite.killTweensOf(this.background);
         TweenLite.killTweensOf(this.ammoTypeIcon);
         TweenLite.killTweensOf(this.textField);
         TweenLite.to(this.background,frame_2,{
            width:15,
            height:28
         });
         TweenLite.to(this.background,frame_1,{
            width:finalWidth,
            height:28,
            delay:frame_2
         });
         TweenLite.to(this.ammoTypeIcon,frame_2,{
            scaleX:1,
            scaleY:1,
            x:5,
            y:4
         });
         TweenLite.to(this.ammoTypeIcon,frame_1,{
            scaleX:1,
            scaleY:1,
            x:finalWidth - 18,
            y:4,
            delay:frame_2
         });
         TweenLite.to(this.textField,frame_2,{
            width:5,
            x:5,
            y:1,
            alpha:0
         });
         TweenLite.to(this.textField,frame_1,{
            width:this.textField.textWidth + 5,
            x:10,
            y:1,
            alpha:1,
            delay:frame_2
         });
      }
      
      public function getFinalWidth() : uint
      {
         return this.textField.textWidth + 37;
      }
      
      public function playOut(force:Boolean = false) : void
      {
         var tween:Tween;
         _state = 0;
         var frame_2:Number = force ? 0 : this.ANIMATION_STAGE_2_TIME / 1000;
         var frame_1:Number = force ? 0 : this.ANIMATION_STAGE_1_TIME / 1000;
         var frameBased:Boolean = false;
         trace("playOut::creatTween",frame_2,frame_1);
         TweenLite.killTweensOf(this.background);
         TweenLite.killTweensOf(this.ammoTypeIcon);
         TweenLite.killTweensOf(this.textField);
         TweenLite.to(this.background,frame_1,{
            width:15,
            height:28
         });
         TweenLite.to(this.background,frame_2,{
            width:15,
            height:23,
            delay:frame_1
         });
         TweenLite.to(this.ammoTypeIcon,frame_1,{
            scaleX:1,
            scaleY:1,
            x:5,
            y:4
         });
         TweenLite.to(this.ammoTypeIcon,frame_2,{
            scaleX:0.8,
            scaleY:0.8,
            x:5,
            y:3,
            delay:frame_1
         });
         TweenLite.to(this.textField,frame_1,{
            alpha:0,
            width:5,
            x:5,
            y:1
         });
      }
   }
}

