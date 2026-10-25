package survarium.controls
{
   import flash.display.Bitmap;
   import flash.display.BitmapData;
   import flash.display.DisplayObject;
   import flash.display.MovieClip;
   import flash.display.PixelSnapping;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import survarium.constants.Colors;
   import survarium.constants.CombatAction;
   
   public class CombatLogItem extends MovieClip
   {
      
      private var _data:Object;
      
      private var _states:Array = ["","kill","suicide","found","lost","bring"];
      
      private var _colorCode:Array = [Colors.OTHER_PLAYER,Colors.ENEMY,Colors.PLAYER];
      
      private static const WALL:BitmapData = new CombatLogWallIcon();

      private static const PAIN:BitmapData = new CombatLogPainIcon();

      // How the victim died, after the weapon (the exe, patch 67: penetration_icon when the killing
      // bullet went through something first, pain_icon when the pain pool killed); in 0.69's order,
      // before the cause icon.
      private var _wallIcon:Bitmap = new Bitmap(WALL,PixelSnapping.ALWAYS);

      private var _painIcon:Bitmap = new Bitmap(PAIN,PixelSnapping.ALWAYS);
      
      public var who_name:TextField;
      
      public var victim_name:TextField;
      
      public var object_icon:MovieClip;
      
      public var extra_icon:MovieClip;
      
      public var mastery_icon:MovieClip;
      
      public function CombatLogItem()
      {
         super();
         this.who_name.autoSize = TextFieldAutoSize.LEFT;
         this.victim_name.autoSize = TextFieldAutoSize.LEFT;
      }
      
      public function setData(data:Object) : void
      {
         _data = data;
         var _action_id:uint = (data.action_id);
         var ll:* = this._states[_action_id];
         gotoAndStop(ll);
         if(this.who_name)
         {
            this.who_name.autoSize = TextFieldAutoSize.LEFT;
         }
         if(this.victim_name)
         {
            this.victim_name.autoSize = TextFieldAutoSize.LEFT;
         }
         this.who_name.text = data.who_name;
         if(this.object_icon)
         {
            this.object_icon.gotoAndStop(data.object_icon + 1);
         }
         if(_action_id == CombatAction.KILL)
         {
            this.victim_name.text = data.victim_name;
         }
         var death:Boolean = _action_id == CombatAction.KILL || _action_id == CombatAction.SUICIDE;
         if(death)
         {
            this.extra_icon.gotoAndStop(data.extra_icon + 1);
            this.mastery_icon.gotoAndStop(data.mastery_icon);
         }
         this.showIcon(this._wallIcon,death && data.penetration_icon == 1);
         this.showIcon(this._painIcon,death && data.pain_icon == 1);
         this.makeLayout();
      }
      
      private function showIcon(icon:DisplayObject, shown:Boolean) : void
      {
         if(shown && icon.parent != this)
         {
            addChild(icon);
         }
         else if(!shown && icon.parent == this)
         {
            removeChild(icon);
         }
      }
      
      // Places a code-made icon at x, centred on the names' line; the x after it.
      private function placeIcon(icon:DisplayObject, x:uint, dx:uint) : uint
      {
         if(icon.parent != this)
         {
            return x;
         }
         icon.x = x;
         icon.y = int(this.who_name.y + (this.who_name.height - icon.height) / 2);
         return x + icon.width + dx;
      }
      
      public function makeLayout() : void
      {
         var x:uint;
         var dx:uint = 5;
         this.who_name.textColor = this._colorCode[this._data.who_team];
         x = this.who_name.x + this.who_name.width + dx;
         if(this.object_icon != null)
         {
            if(this.object_icon)
            {
               this.object_icon.x = x;
            }
            x += this.object_icon.width + dx;
         }
         x = this.placeIcon(this._wallIcon,x,dx);
         x = this.placeIcon(this._painIcon,x,dx);
         if(this._data.extra_icon != 0 && this.extra_icon)
         {
            this.extra_icon.x = x;
            x += this.extra_icon.width + dx;
         }
         if(this._data.action_id == CombatAction.KILL)
         {
            this.victim_name.textColor = this._colorCode[this._data.victim_team];
            this.victim_name.x = x;
            x += this.victim_name.width + dx;
         }
         if(this._data.mastery_icon != 0 && this.mastery_icon)
         {
            this.mastery_icon.x = x;
            x += this.mastery_icon.width + dx;
         }
         this.x = 190 - width;
      }
      
      override public function toString() : String
      {
         return "[CombatLogLayout " + "]";
      }
   }
}

