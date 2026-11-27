package survarium.controls
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.TimerEvent;
   import flash.geom.ColorTransform;
   import flash.geom.Rectangle;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.utils.Timer;
   import scaleform.clik.controls.Label;
   
   public class HUDMarker extends MovieClip
   {
      
      protected static var _labelClass:Class;
      
      protected static var _labelShift:int = 5;
      
      public var textField:TextField;
      
      public var icon:MovieClip;
      
      public var icon_box:MovieClip;
      
      public var baseScale:Number;
      
      public var team:uint;
      
      public var kind:uint;
      
      protected var _tm:Timer = new Timer(500,1);
      
      protected var _visibleTarget:Boolean = false;
      
      protected var _labelRenderer:Label;

      // Freeze Tag (patch 160, docs/notes/freeze-tag.md): a frozen player's name and arrow are drawn in
      // ice blue (the hud movie's FrozenPlaces.COLOR, the same value). Every colour the movie or the game
      // gives them is kept as their own and comes back on thaw; a name set while frozen stays blue (the
      // names are plain text: .text).
      public static const FROZEN_COLOR:uint = 0x66C2FF;

      protected var _nameColor:uint;

      // The arrow's colour the game gives (render_name's rgb, set_player_color), null before any: the
      // movie's own (its parts have none).
      protected var _iconColour:ColorTransform = null;

      protected var _frozen:Boolean = false;
      // The game's carrier mark (modifier_visible, set_player_icon_container): a battery's carrier in
      // the game's modes, the artefact's in Artifact Hunt (artefactCarry).
      protected var _carrying:Boolean = false;

      // Artifact Hunt (HUDIconLayer.carriersShown): the mark is the artefact's (ArtefactMark), not
      // the battery's box.
      protected var _artefactCarry:Boolean = false;

      protected var _artefactMark:ArtefactMark;
      
      public function HUDMarker()
      {
         super();
         if(this.textField)
         {
            this._nameColor = this.textField.textColor;
         }
         if(this.icon_box)
         {
            this.icon_box.visible = false;
         }
         if(this.icon && this.icon.glow)
         {
            this.icon.glow.visible = false;
         }
      }
      
      public static function setLabelRendererClass(value:Class) : void
      {
         _labelClass = value;
      }
      
      public function set type(value:uint) : void
      {
         this.icon.gotoAndStop(value + 1);
         // another frame's arrow: a frozen one stays blue
         if(this._frozen)
         {
            this.applyIconColour();
         }
      }

      public function set iconColour(value:ColorTransform) : void
      {
         this._iconColour = value;
         this.applyIconColour();
      }
      
      public function set label(value:String) : void
      {
         if(value)
         {
            if(!this._labelRenderer)
            {
               _labelRenderer = new _labelClass();
            }
            this._labelRenderer.autoSize = TextFieldAutoSize.LEFT;
            this._labelRenderer.text = value;
            addChild(this._labelRenderer);
            this._labelRenderer.validateNow();
            this._labelRenderer.x = this.icon.x + this.icon.width + _labelShift;
            this._labelRenderer.y = this.textField.y - this._labelRenderer.height - _labelShift;
         }
         else if(this._labelRenderer)
         {
            removeChild(this._labelRenderer);
            _labelRenderer = null;
         }
      }
      
      // The name's own colour (the team's, as the game sends it).
      public function set nameColor(value:uint) : void
      {
         this._nameColor = value;
         this.applyNameColor();
      }

      public function setName(value:String) : void
      {
         if(this.textField.text != value)
         {
            this.textField.text = value;
         }
         if(this._frozen)
         {
            this.applyNameColor();
         }
      }

      public function set frozen(value:Boolean) : void
      {
         if(value == this._frozen)
         {
            return;
         }
         this._frozen = value;
         this.applyNameColor();
         this.applyIconColour();
      }

      protected function applyNameColor() : void
      {
         if(this.textField)
         {
            this.textField.textColor = this._frozen ? FROZEN_COLOR : this._nameColor;
         }
      }

      // Every part of the arrow named renderer (each frame of the icon has two, the arrow and its inner
      // part; icon.renderer names one of them only): the ice blue while frozen, its alpha the game's;
      // else the game's colour, or none.
      protected function applyIconColour() : void
      {
         var colour:ColorTransform = this._iconColour != null ? this._iconColour : new ColorTransform();
         if(this._frozen)
         {
            var ice:ColorTransform = new ColorTransform();
            ice.color = FROZEN_COLOR;
            ice.alphaMultiplier = colour.alphaMultiplier;
            ice.alphaOffset = colour.alphaOffset;
            colour = ice;
         }
         for(var i:int = 0; this.icon != null && i < this.icon.numChildren; i++)
         {
            if(this.icon.getChildAt(i).name == "renderer")
            {
               this.icon.getChildAt(i).transform.colorTransform = colour;
            }
         }
      }

      public function set glow(value:Boolean) : void
      {
         trace("set_glow::",value);
         if(!this.icon || !this.icon.glow)
         {
            return;
         }
         this.icon.glow.visible = value;
         this.icon.glow.gotoAndPlay(2);
      }
      
      public function set container(value:Boolean) : void
      {
         this._carrying = value;
         this.showCarryMark();
      }

      public function get carrying() : Boolean
      {
         return this._carrying;
      }

      public function set artefactCarry(value:Boolean) : void
      {
         this._artefactCarry = value;
         this.showCarryMark();
      }

      public function get artefactMarkShown() : Boolean
      {
         return this._artefactMark != null && this._artefactMark.visible;
      }

      // The battery's box, or in Artifact Hunt the artefact's orb where the box is.
      protected function showCarryMark() : void
      {
         if(this.icon_box)
         {
            this.icon_box.visible = this._carrying && !this._artefactCarry;
         }
         if(this._artefactCarry && this._carrying && !this._artefactMark && this.icon_box)
         {
            var box:Rectangle = this.icon_box.getBounds(this);
            _artefactMark = new ArtefactMark(Math.max(box.width,box.height) / 2);
            this._artefactMark.x = box.x + box.width / 2;
            this._artefactMark.y = box.y + box.height / 2;
            addChild(this._artefactMark);
         }
         if(this._artefactMark)
         {
            this._artefactMark.visible = this._artefactCarry && this._carrying;
         }
      }
      
      public function set delay(value:uint) : void
      {
         this._tm.delay = value;
      }
      
      public function showName() : *
      {
         if(this._visibleTarget == true)
         {
            return;
         }
         _visibleTarget = true;
         if(!this.textField.visible)
         {
            this._tm.reset();
            this._tm.start();
            this._tm.addEventListener(TimerEvent.TIMER_COMPLETE,this.handleTimerShow);
         }
         else
         {
            this._tm.removeEventListener(TimerEvent.TIMER_COMPLETE,this.handleTimerHide);
         }
      }
      
      public function hideName() : *
      {
         if(this._visibleTarget == false)
         {
            return;
         }
         _visibleTarget = false;
         if(this.textField.visible)
         {
            this._tm.reset();
            this._tm.start();
            this._tm.addEventListener(TimerEvent.TIMER_COMPLETE,this.handleTimerHide);
         }
         else
         {
            this._tm.removeEventListener(TimerEvent.TIMER_COMPLETE,this.handleTimerShow);
         }
      }
      
      public function resetTimer() : void
      {
         if(this._tm.hasEventListener(TimerEvent.TIMER_COMPLETE))
         {
            this._tm.removeEventListener(TimerEvent.TIMER_COMPLETE,this.handleTimerShow);
            this._tm.removeEventListener(TimerEvent.TIMER_COMPLETE,this.handleTimerHide);
         }
      }
      
      protected function handleTimerShow(e:Event) : void
      {
         this._tm.removeEventListener(TimerEvent.TIMER_COMPLETE,this.handleTimerShow);
         this.textField.visible = true;
      }
      
      protected function handleTimerHide(e:Event) : void
      {
         this._tm.removeEventListener(TimerEvent.TIMER_COMPLETE,this.handleTimerHide);
         this.textField.visible = false;
      }
   }
}

