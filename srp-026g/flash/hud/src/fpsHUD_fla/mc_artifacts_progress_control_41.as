package fpsHUD_fla
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
   import survarium.constants.Team;
   
   public dynamic class mc_artifacts_progress_control_41 extends MovieClip
   {
      
      public var slots:Array;
      
      public var team:uint;
      
      public var renderer_width:uint;
      
      public function mc_artifacts_progress_control_41()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      public function set_progress(value:uint, max:uint) : *
      {
         for(var i:uint = 0; i < max; i++)
         {
            var art:ArtifactMC;
            if(!this.slots[i])
            {
               art = new ArtifactMC();
               art.x = i * this.renderer_width;
               this.slots[i] = art;
               addChild(art);
               art.gotoAndStop(1);
               art.scaleX = scaleX;
            }
            else
            {
               art = this.slots[i];
            }
            if(this.team == Team.FIRST)
            {
               if(i < value)
               {
                  art.gotoAndStop(2);
               }
               else if(art.currentFrame == 2)
               {
                  art.gotoAndStop(1);
               }
            }
            else if(max - i - 1 < value)
            {
               art.gotoAndStop(3);
            }
            else if(art.currentFrame == 3)
            {
               art.gotoAndStop(1);
            }
         }
         for(i = max; i < this.slots.length; i++)
         {
            art = this.slots[i - 1];
            if(art)
            {
               removeChild(art);
            }
            delete this.slots[i - 1];
         }
         x = -(max * this.renderer_width) / 2;
         parent["team_1_progress"].x = x - 20;
         parent["team_2_progress"].x = x + width + 14;
      }
      
      internal function frame1() : *
      {
         slots = new Array();
         team = 0;
         renderer_width = 20;
      }
   }
}

