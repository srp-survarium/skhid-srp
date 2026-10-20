package
{
   import adobe.utils.*;
   import flash.accessibility.*;
   import flash.desktop.*;
   import flash.display.*;
   import flash.errors.*;
   import flash.events.*;
   import flash.external.*;
   import flash.filters.*;
   import flash.geom.*;
   import flash.globalization.*;
   import flash.media.*;
   import flash.net.*;
   import flash.net.drm.*;
   import flash.printing.*;
   import flash.profiler.*;
   import flash.sampler.*;
   import flash.sensors.*;
   import flash.system.*;
   import flash.text.*;
   import flash.text.engine.*;
   import flash.text.ime.*;
   import flash.ui.*;
   import flash.utils.*;
   import flash.xml.*;
   import survarium.messages.MessageDailyReward;
   
   public dynamic class SrvMessageDailyReward extends MessageDailyReward
   {
      
      public function SrvMessageDailyReward()
      {
         super();
         this.__setProp_rewardsList_SrvMessageDailyReward_();
      }
      
      internal function __setProp_rewardsList_SrvMessageDailyReward_() : *
      {
         try
         {
            rewardsList["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         rewardsList.autoSize = "none";
         rewardsList.buttonWidth = 0;
         rewardsList.direction = "horizontal";
         rewardsList.enabled = true;
         rewardsList.enableInitCallback = false;
         rewardsList.focusable = true;
         rewardsList.itemRendererName = "SrvDroppedItemRenderer";
         rewardsList.spacing = 3;
         rewardsList.visible = true;
         try
         {
            rewardsList["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

