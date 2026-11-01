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
      
      private static const RICOCHET:BitmapData = new CombatLogRicochetIcon();
      
      private static const PAIN:BitmapData = new CombatLogPainIcon();
      
      // How the victim died, after the weapon (the exe, patch 67, 0.69d0's rules: penetration_icon when
      // the killing bullet went through something first, ricochet_icon when it ricocheted; pain_icon
      // when the pain pool killed); in 0.69d0's order, before the cause icon.
      private var _wallIcon:Bitmap = new Bitmap(WALL,PixelSnapping.ALWAYS);
      
      private var _ricochetIcon:Bitmap = new Bitmap(RICOCHET,PixelSnapping.ALWAYS);
      
      private var _painIcon:Bitmap = new Bitmap(PAIN,PixelSnapping.ALWAYS);
      
      // A weapon's icon is its config's ui_desc/combat_log_icon: frame (icon + 1) of object_icon, whose
      // 53 frames end at icon 52. The builds the ported weapons come from number theirs on from 53
      // (0.27d0 to 0.31a2 alike): those icons are these bitmaps, by icon, shown in the frame's place.
      private static const MORE_WEAPONS:Object = {53:CombatLogWeapon53,54:CombatLogWeapon54,55:CombatLogWeapon55,56:CombatLogWeapon56,57:CombatLogWeapon57,58:CombatLogWeapon58,59:CombatLogWeapon59,60:CombatLogWeapon60,61:CombatLogWeapon61,62:CombatLogWeapon62,63:CombatLogWeapon63,64:CombatLogWeapon64,65:CombatLogWeapon65,66:CombatLogWeapon66,67:CombatLogWeapon67,68:CombatLogWeapon68,70:CombatLogWeapon70,71:CombatLogWeapon71,72:CombatLogWeapon72,74:CombatLogWeapon74};
      
      private static var _moreWeapons:Object = {};
      
      private var _weaponIcon:Bitmap = new Bitmap(null,PixelSnapping.ALWAYS);
      
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
         var weapon:BitmapData = null;
         if(this.object_icon)
         {
            this.object_icon.gotoAndStop(data.object_icon + 1);
            if(_action_id == CombatAction.KILL)
            {
               weapon = moreWeapon(data.object_icon);
            }
            this.object_icon.visible = weapon == null;
         }
         this._weaponIcon.bitmapData = weapon;
         this.showIcon(this._weaponIcon,weapon != null);
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
         this.showIcon(this._ricochetIcon,death && data.ricochet_icon == 1);
         this.showIcon(this._painIcon,death && data.pain_icon == 1);
         this.makeLayout();
      }
      
      // The bitmap of `icon`, made once; null for an icon of the clip's or one with no bitmap.
      private static function moreWeapon(icon:int) : BitmapData
      {
         if(!(icon in MORE_WEAPONS))
         {
            return null;
         }
         if(!_moreWeapons[icon])
         {
            _moreWeapons[icon] = new MORE_WEAPONS[icon]();
         }
         return _moreWeapons[icon];
      }
      
      // What shows the weapon: object_icon, or the bitmap in its place; null on a suicide row.
      private function get weaponMark() : DisplayObject
      {
         return this._weaponIcon.parent == this ? this._weaponIcon : this.object_icon;
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
      
      // Places a code-made icon at x, centred on the weapon's icon (not above the row); the x after it.
      // A suicide row has no weapon icon (its frame removes object_icon, which is null there): the
      // pain icon is centred on the cause icon instead.
      private function placeIcon(icon:DisplayObject, x:uint, dx:uint) : uint
      {
         if(icon.parent != this)
         {
            return x;
         }
         var mark:DisplayObject = this.weaponMark != null ? this.weaponMark : this.extra_icon;
         icon.x = x;
         icon.y = mark == null ? 0 : Math.max(0,int(mark.y + (mark.height - icon.height) / 2));
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
            this.object_icon.x = x;
         }
         if(this._weaponIcon.parent == this)
         {
            // centred where the clip's frames centre theirs (12.5 px down it), never above it
            this._weaponIcon.x = x;
            this._weaponIcon.y = this.object_icon.y + Math.max(0,int((25 - this._weaponIcon.height) / 2));
         }
         if(this.weaponMark != null)
         {
            x += this.weaponMark.width + dx;
         }
         x = this.placeIcon(this._wallIcon,x,dx);
         x = this.placeIcon(this._ricochetIcon,x,dx);
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

