package survarium.buttons
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.text.TextField;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.ListItemRenderer;
   import scaleform.gfx.MouseEventEx;
   import survarium.char.SkillProgressIndicator;
   import survarium.constants.Events;
   import survarium.interfaces.ISkillRenderer;
   
   public class SkillListRenderer extends ListItemRenderer implements ISkillRenderer
   {
      
      protected static const LEVELS:Array = ["I","II","III","IV","V","VI"];
      
      public var hoverSprite:Sprite;
      
      public var overSprite:Sprite;
      
      public var background:MovieClip;
      
      public var textFieldLabel:TextField;
      
      public var textFieldLevel:TextField;
      
      public var skillProgressIndicator:SkillProgressIndicator;
      
      public var iconContainer:MovieClip;
      
      public var lockSprite:MovieClip;
      
      public var lockSpriteRed:MovieClip;
      
      protected var _perksIconsContainer:MovieClip;
      
      protected var _complete:Boolean = false;
      
      protected var _locked:Boolean = false;
      
      private var _unlockable:Boolean = false;
      
      public function SkillListRenderer()
      {
         super();
      }
      
      override public function set enabled(param1:Boolean) : void
      {
         super.enabled = param1;
         this.lockSprite.visible = enabled == false;
         this.updateDescription();
      }
      
      override public function set data(param1:Object) : void
      {
         super.data = param1;
         trace("SLR:data",param1 ? param1.enabled : "null");
         if(param1 && param1.enabled != undefined)
         {
            enabled = param1.enabled && !this._locked;
         }
         this.updateDescription();
         invalidateData();
      }
      
      public function showLock(param1:Boolean) : void
      {
         this.lockSpriteRed.visible = this._locked || param1;
      }
      
      public function set unlockable(param1:Boolean) : void
      {
         _unlockable = param1;
         if(!this._unlockable && this.hoverSprite.visible)
         {
            this.hoverSprite.visible = false;
         }
      }
      
      public function lock(param1:Boolean) : void
      {
         _locked = param1;
         this.lockSpriteRed.visible = this._locked;
         var _loc2_:Boolean = true;
         if(_data && _data.enabled != undefined)
         {
            _loc2_ = _data.enabled;
         }
         enabled = _loc2_ && !this._locked;
      }
      
      private function updateDescription() : void
      {
         trace("SLR_updateDescription");
         if(_data)
         {
            _data.description_1 = _data.name;
            if(_data.description_2 is Array)
            {
               _data.description_2_string = _data.description_2[_data.newValue];
            }
            else
            {
               _data.description_2_string = null;
            }
         }
         invalidateData();
      }
      
      override protected function handleMouseRollOver(param1:MouseEvent) : void
      {
         super.handleMouseRollOver(param1);
         this.hoverSprite.visible = enabled && !this._complete && !this._locked && this._unlockable;
         this.overSprite.visible = enabled && !this._locked;
         this.background.gotoAndStop(enabled ? 2 : 1);
         if(_data && _data.antiskills && !this._locked)
         {
            dispatchEvent(new Event(Events.SHOW_ANTISKILLS,true));
         }
      }
      
      override protected function handleMouseRollOut(param1:MouseEvent) : void
      {
         super.handleMouseRollOut(param1);
         this.hoverSprite.visible = false;
         this.overSprite.visible = false;
         this.background.gotoAndStop(1);
         if(_data && _data.antiskills && !this._locked)
         {
            dispatchEvent(new Event(Events.HIDE_ANTISKILLS,true));
         }
      }
      
      override protected function configUI() : void
      {
         trace("config_slr",this.background,this.hoverSprite);
         super.configUI();
         this.background.gotoAndStop(1);
         preventAutosizing = true;
         this.hoverSprite.visible = false;
         this.overSprite.visible = false;
         this.lockSpriteRed.visible = this._locked;
         this.skillProgressIndicator.registerRenderer(SkillProgressIcon);
         addEventListener(MouseEvent.CLICK,this.onSliderUpdate);
         tooltipRendererName = "SrvPerkTipRenderer";
         mouseChildren = false;
         constraintsDisabled = true;
         _perksIconsContainer = new PerkIconContainer();
         this._perksIconsContainer.gotoAndStop(1);
         this._perksIconsContainer.icons.gotoAndStop(1);
         this.iconContainer.addChildAt(this._perksIconsContainer,this.iconContainer.numChildren - 1);
      }
      
      private function onSliderUpdate(param1:MouseEventEx) : void
      {
         if((enabled && data && !isInvalid(InvalidationType.DATA)) && !this._locked && !this.lockSpriteRed.visible)
         {
            if(param1.buttonIdx == 0)
            {
               if(!this._complete && this._unlockable)
               {
                  this.skillProgressIndicator.value += 1;
                  _data.newValue = this.skillProgressIndicator.value;
               }
            }
            else if(this.skillProgressIndicator.value > _data.value)
            {
               this.skillProgressIndicator.value -= 1;
               _data.newValue = this.skillProgressIndicator.value;
               this.hoverSprite.visible = true;
            }
            dispatchEvent(new Event(Events.SKILL_UPDATE,true));
            this.updateDescription();
         }
      }
      
      override protected function draw() : void
      {
         trace("SLR_RENDRAW::",_data ? _data.tipText : null);
         if(isInvalid(InvalidationType.DATA))
         {
            this.updateData();
         }
         super.draw();
      }
      
      protected function updateData() : void
      {
         var _loc1_:uint;
         var _loc2_:int;
         mouseChildren = true;
         visible = _data != null;
         if(_data)
         {
            _loc1_ = (_data.newValue);
            if(!enabled && _loc1_)
            {
               _loc1_ = 0;
               _data.newValue = 0;
               dispatchEvent(new Event(Events.SKILL_UPDATE,true));
            }
            this.textFieldLabel.text = _data.name;
            this.textFieldLevel.text = LEVELS[_data.level - 1];
            this.skillProgressIndicator.minimum = 0;
            this.skillProgressIndicator.visible = true;
            _loc2_ = (_data.maxValue);
            _complete = _loc2_ == _loc1_;
            if(this.hoverSprite.visible && this._complete)
            {
               this.hoverSprite.visible = false;
            }
            _data.resize = Boolean(_loc2_ > 1);
            trace("slr_update_data",_data.name,_data.enabled,_loc1_,_loc2_,this._complete);
            this.skillProgressIndicator.maximum = _loc2_;
            this.skillProgressIndicator.value = _loc1_;
            this.skillProgressIndicator.enabled = enabled;
            this.skillProgressIndicator.invalidateData();
            this.skillProgressIndicator.validateNow();
            this.background.sprite.gotoAndStop(_data.branch + 1);
            this._perksIconsContainer.gotoAndStop(_data.branch + 1);
            this._perksIconsContainer.icons.gotoAndStop(_data.icon + 1);
            if(enabled)
            {
               this._perksIconsContainer.alpha = _loc1_ > 0 ? 1 : 0.4;
               this.skillProgressIndicator.alpha = 1;
            }
            else
            {
               this._perksIconsContainer.alpha = 0.6;
               this.skillProgressIndicator.alpha = this._perksIconsContainer.alpha;
            }
         }
      }
   }
}

