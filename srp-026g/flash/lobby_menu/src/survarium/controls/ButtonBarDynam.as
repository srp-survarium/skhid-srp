package survarium.controls
{
   import flash.text.TextFieldAutoSize;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ButtonBar;
   
   public class ButtonBarDynam extends ButtonBar
   {
      
      public var useRect:Boolean = true;
      
      public function ButtonBarDynam()
      {
         super();
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
               _loc5_ = _renderers.length - 1;
               if(container.contains(_renderers[_loc5_]))
               {
                  container.removeChild(_renderers[_loc5_]);
               }
               _renderers.splice(_loc5_--,1);
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
         var _loc5_:int;
         var _loc6_:Button;
         var _loc7_:Boolean;
         var _loc8_:int;
         var _loc9_:Button;
         while(_loc4_ < _dataProvider.length && _loc3_ == -1)
         {
            _loc7_ = false;
            if(_loc4_ < _renderers.length)
            {
               _loc6_ = _renderers[_loc4_];
            }
            else
            {
               _loc6_ = new _itemRendererClass();
               setupRenderer(_loc6_,_loc4_);
               _loc7_ = true;
            }
            populateRendererData(_loc6_,_loc4_);
            if(_autoSize == TextFieldAutoSize.NONE && _buttonWidth > 0)
            {
               _loc6_.width = _buttonWidth;
            }
            else if(_autoSize != TextFieldAutoSize.NONE)
            {
               _loc6_.autoSize = _autoSize;
            }
            _loc6_.validateNow();
            if(_direction == DIRECTION_HORIZONTAL)
            {
               if(_width > _loc6_.width + _spacing + _loc1_ || _maxFit)
               {
                  _loc6_.y = 0;
                  _loc6_.x = _loc1_;
                  _loc1_ += (this.useRect ? _loc6_.getRect(_loc6_).width : _loc6_.width) + _spacing;
               }
               else
               {
                  _loc3_ = _loc4_;
                  _loc6_ = null;
               }
            }
            else if(_height > _loc6_.height + _spacing + _loc2_ || _maxFit)
            {
               _loc6_.x = 0;
               _loc6_.y = _loc2_;
               _loc2_ += _loc6_.getRect(_loc6_).height + _spacing;
            }
            else
            {
               _loc3_ = _loc4_;
               _loc6_ = null;
            }
            if(_loc7_ && _loc6_ != null)
            {
               _loc6_.group = _group;
               container.addChild(_loc6_);
               _renderers.push(_loc6_);
            }
            _loc4_++;
         }
         if(_loc3_ > -1)
         {
            _loc8_ = _renderers.length - 1;
            while(_loc8_ >= _loc3_)
            {
               _loc9_ = _renderers[_loc8_];
               if(_loc9_)
               {
                  if(container.contains(_loc9_))
                  {
                     container.removeChild(_loc9_);
                  }
                  _renderers.splice(_loc8_,1);
               }
               _loc8_--;
            }
         }
         selectedIndex = Math.min(_dataProvider.length - 1,_selectedIndex);
      }
   }
}

