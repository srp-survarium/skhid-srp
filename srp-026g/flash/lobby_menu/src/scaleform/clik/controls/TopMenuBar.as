package scaleform.clik.controls
{
   import flash.events.Event;
   import flash.text.TextFieldAutoSize;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.constants.NavigationCode;
   import scaleform.clik.events.ButtonBarEvent;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.ui.InputDetails;
   
   public class TopMenuBar extends ButtonBar
   {
      
      protected var _groupRenderers:uint = 3;
      
      protected var _groupMargin:uint = 210;
      
      public function TopMenuBar()
      {
         super();
      }
      
      override public function handleInput(param1:InputEvent) : void
      {
         if(param1.handled)
         {
            return;
         }
         var _loc2_:Button = _renderers[_selectedIndex] as Button;
         if(_loc2_ != null)
         {
            _loc2_.handleInput(param1);
            if(param1.handled)
            {
               return;
            }
         }
         var _loc3_:InputDetails = param1.details;
         var _loc4_:Boolean = _loc3_.value == InputValue.KEY_DOWN || _loc3_.value == InputValue.KEY_HOLD;
         if(!_loc4_)
         {
            return;
         }
         var _loc5_:Boolean = false;
         var _loc6_:Number;
         switch(_loc3_.navEquivalent)
         {
            case NavigationCode.LEFT:
               if(_direction == DIRECTION_HORIZONTAL)
               {
                  _loc6_ = this.getPrevIndex();
                  _loc5_ = true;
               }
               break;
            case NavigationCode.RIGHT:
               if(_direction == DIRECTION_HORIZONTAL)
               {
                  _loc6_ = this.getNextIndex();
                  _loc5_ = true;
               }
               break;
            case NavigationCode.UP:
               if(_direction == DIRECTION_VERTICAL)
               {
                  _loc6_ = this.getPrevIndex();
                  _loc5_ = true;
               }
               break;
            case NavigationCode.DOWN:
               if(_direction == DIRECTION_VERTICAL)
               {
                  _loc6_ = this.getNextIndex();
                  _loc5_ = true;
               }
               break;
            default:
               break;
         }
         if(_loc5_)
         {
            _loc6_ = Math.max(0,Math.min(_dataProvider.length - 1,_loc6_));
            if(_loc6_ != _selectedIndex)
            {
               selectedIndex = _loc6_;
               param1.handled = true;
            }
         }
      }
      
      protected function getPrevIndex() : Number
      {
         var _loc1_:Button;
         var _loc2_:int;
         if(_selectedIndex != 0)
         {
            _loc2_ = _selectedIndex - 1;
            while(_loc2_ >= 0)
            {
               _loc1_ = _renderers[_loc2_];
               if(_loc1_.enabled)
               {
                  return _loc2_;
               }
               _loc2_--;
            }
         }
         return _selectedIndex;
      }
      
      protected function getNextIndex() : Number
      {
         var _loc1_:Button;
         var _loc2_:int;
         if(_selectedIndex != _renderers.length)
         {
            _loc2_ = _selectedIndex + 1;
            while(_loc2_ < _renderers.length)
            {
               _loc1_ = _renderers[_loc2_];
               if(_loc1_.enabled)
               {
                  return _loc2_;
               }
               _loc2_++;
            }
         }
         return _selectedIndex;
      }
      
      override protected function updateRenderers() : void
      {
         var _loc1_:Number = 0;
         var _loc2_:Number = 0;
         var _loc3_:int = -1;
         if(_renderers[0] is _itemRendererClass)
         {
            while(_renderers.length > _dataProvider.length)
            {
               _loc6_ = _renderers.length - 1;
               if(container.contains(_renderers[_loc6_]))
               {
                  container.removeChild(_renderers[_loc6_]);
               }
               _renderers.splice(_loc6_--,1);
            }
         }
         else
         {
            while(container.numChildren > 0)
            {
               container.removeChildAt(0);
            }
            _renderers.length = 0;
         }
         var _loc4_:uint = 0;
         var _loc5_:uint = 0;
         var _loc6_:int;
         var _loc7_:Button;
         var _loc8_:Boolean;
         var _loc9_:int;
         var _loc10_:Button;
         while(_loc5_ < _dataProvider.length && _loc3_ == -1)
         {
            _loc8_ = false;
            if(_loc5_ < _renderers.length)
            {
               _loc7_ = _renderers[_loc5_];
            }
            else
            {
               _loc7_ = new _itemRendererClass();
               setupRenderer(_loc7_,_loc5_);
               _loc8_ = true;
            }
            populateRendererData(_loc7_,_loc5_);
            if(_autoSize == TextFieldAutoSize.NONE && _buttonWidth > 0)
            {
               _loc7_.width = _buttonWidth;
            }
            else if(_autoSize != TextFieldAutoSize.NONE)
            {
               _loc7_.autoSize = _autoSize;
            }
            _loc7_.validateNow();
            if(_direction == DIRECTION_HORIZONTAL)
            {
               if(_width > _loc7_.width + _spacing + _loc1_ || _maxFit)
               {
                  _loc7_.y = 0;
                  _loc7_.x = _loc1_;
                  _loc1_ += _loc7_.width + _spacing;
                  if(_loc4_ != int((_loc5_ + 1) / this._groupRenderers) * this._groupMargin)
                  {
                     _loc4_ = int((_loc5_ + 1) / this._groupRenderers) * this._groupMargin;
                     _loc1_ += int((_loc5_ + 1) / this._groupRenderers) * this._groupMargin;
                  }
               }
               else
               {
                  _loc3_ = _loc5_;
                  _loc7_ = null;
               }
            }
            else if(_height > _loc7_.height + _spacing + _loc2_)
            {
               _loc7_.x = 0;
               _loc7_.y = _loc2_;
               _loc2_ += _loc7_.height + _spacing;
            }
            else
            {
               _loc3_ = _loc5_;
               _loc7_ = null;
            }
            if(_loc8_ && _loc7_ != null)
            {
               _loc7_.group = _group;
               container.addChild(_loc7_);
               _renderers.push(_loc7_);
            }
            _loc5_++;
         }
         if(_loc3_ > -1)
         {
            _loc9_ = _renderers.length - 1;
            while(_loc9_ >= _loc3_)
            {
               _loc10_ = _renderers[_loc9_];
               if(_loc10_)
               {
                  if(container.contains(_loc10_))
                  {
                     container.removeChild(_loc10_);
                  }
                  _renderers.splice(_loc9_,1);
               }
               _loc9_--;
            }
         }
         selectedIndex = Math.min(_dataProvider.length - 1,_selectedIndex);
      }
      
      override protected function handleButtonGroupChange(param1:Event) : void
      {
         if(_group.selectedIndex != selectedIndex)
         {
            selectedIndex = _group.selectedIndex;
            dispatchEvent(new ButtonBarEvent(ButtonBarEvent.BUTTON_SELECT,false,true,_selectedIndex,param1.target as Button));
         }
         else
         {
            selectedIndex = -1;
            dispatchEvent(new ButtonBarEvent(ButtonBarEvent.BUTTON_SELECT,false,true,_selectedIndex,param1.target as Button));
         }
      }
   }
}

